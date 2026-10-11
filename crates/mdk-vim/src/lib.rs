//! A small Vim, written for mdk's card editor and independent of it.
//!
//! [`Vim`] is a state machine. Feed it keys with [`Vim::handle`] and it edits anything that
//! implements [`VimBuffer`]. It covers normal, insert and visual modes (by character and by
//! line), the motions `h j k l w b e 0 ^ $ gg G` with counts, `i a I A o O`, `x dd yy p P`,
//! `u` and `Ctrl-r`, the operators `d c y` with any of those motions, `d y c > <` in visual
//! mode, `/` with `n` and `N`, and `:w` and `:q`. No registers, macros, marks or text objects.

use std::ops::Range;

/// The text a [`Vim`] edits. Offsets are byte offsets on character boundaries.
pub trait VimBuffer {
    fn text(&self) -> String;
    fn cursor(&self) -> usize;
    fn set_cursor(&mut self, pos: usize);
    /// Show a visual-mode selection.
    fn set_selection(&mut self, range: Range<usize>);
    /// Replace a range with text, as one undo step.
    fn replace(&mut self, range: Range<usize>, text: &str);
    fn undo(&mut self);
    fn redo(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
}

/// What became of a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    /// Vim used the key. The editor must not see it.
    Yes,
    /// Insert mode: the editor should type the key as usual.
    No,
    /// `:q` was entered. Close the card.
    Close,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// The target character is not part of the range (`w`, `h`, `0`).
    Exclusive,
    /// It is (`e`, `$`).
    Inclusive,
    /// Whole lines (`j`, `G`).
    Linewise,
}

/// The buffer's text as characters, with the line arithmetic Vim needs.
struct Doc {
    chars: Vec<char>,
}

impl Doc {
    fn of(buf: &dyn VimBuffer) -> (Doc, usize) {
        let text = buf.text();
        let cursor = text[..buf.cursor().min(text.len())].chars().count();
        (
            Doc {
                chars: text.chars().collect(),
            },
            cursor,
        )
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    /// The byte offset of a character index.
    fn byte(&self, index: usize) -> usize {
        self.chars[..index.min(self.len())]
            .iter()
            .map(|c| c.len_utf8())
            .sum()
    }

    fn bytes(&self, range: Range<usize>) -> Range<usize> {
        self.byte(range.start)..self.byte(range.end)
    }

    fn slice(&self, range: Range<usize>) -> String {
        self.chars[range].iter().collect()
    }

    fn line_start(&self, pos: usize) -> usize {
        self.chars[..pos.min(self.len())]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1)
    }

    /// The index of the line's line break, or the end of the text.
    fn line_end(&self, pos: usize) -> usize {
        let pos = pos.min(self.len());
        self.chars[pos..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(self.len(), |i| pos + i)
    }

    fn first_non_blank(&self, pos: usize) -> usize {
        let (start, end) = (self.line_start(pos), self.line_end(pos));
        (start..end)
            .find(|&i| !self.chars[i].is_whitespace())
            .unwrap_or(end)
    }

    /// Normal mode keeps the cursor on a character, never after the last one of a line.
    fn clamp(&self, pos: usize) -> usize {
        let (start, end) = (self.line_start(pos), self.line_end(pos));
        pos.min(end.saturating_sub(1)).max(start)
    }

    /// 0 for blank, 1 for word characters, 2 for punctuation.
    fn class(&self, index: usize) -> u8 {
        match self.chars.get(index) {
            None => 0,
            Some(c) if c.is_whitespace() => 0,
            Some(c) if c.is_alphanumeric() || *c == '_' => 1,
            Some(_) => 2,
        }
    }

    fn empty_line(&self, pos: usize) -> bool {
        pos < self.len() && self.line_start(pos) == pos && self.chars[pos] == '\n'
    }

    fn word_forward(&self, mut pos: usize) -> usize {
        let class = self.class(pos);
        if class != 0 {
            while pos < self.len() && self.class(pos) == class {
                pos += 1;
            }
        }
        // Blanks are skipped, but an empty line is a place to stop.
        while pos < self.len() && self.class(pos) == 0 && !(self.empty_line(pos) && class == 0) {
            if self.chars[pos] == '\n' && self.empty_line(pos + 1) {
                return pos + 1;
            }
            pos += 1;
        }
        pos
    }

    fn word_back(&self, mut pos: usize) -> usize {
        while pos > 0 && self.class(pos - 1) == 0 {
            pos -= 1;
        }
        let class = self.class(pos.saturating_sub(1));
        while pos > 0 && self.class(pos - 1) == class && class != 0 {
            pos -= 1;
        }
        pos
    }

    fn word_end(&self, mut pos: usize) -> usize {
        pos += 1;
        while pos < self.len() && self.class(pos) == 0 {
            pos += 1;
        }
        let class = self.class(pos);
        while pos + 1 < self.len() && self.class(pos + 1) == class && class != 0 {
            pos += 1;
        }
        pos.min(self.len().saturating_sub(1))
    }

    /// The start of the line `n` lines below (or above) `pos`'s line, stopping at the ends.
    fn line_offset(&self, pos: usize, down: bool, n: usize) -> usize {
        let mut start = self.line_start(pos);
        for _ in 0..n {
            if down {
                let end = self.line_end(start);
                if end >= self.len() {
                    break;
                }
                start = end + 1;
            } else {
                if start == 0 {
                    break;
                }
                start = self.line_start(start - 1);
            }
        }
        start
    }

    /// The start of line `number`, counted from 1 and held inside the text.
    fn line_number(&self, number: usize) -> usize {
        self.line_offset(0, true, number.saturating_sub(1))
    }

    fn find(&self, pattern: &[char], from: usize, forward: bool) -> Option<usize> {
        let last = self.len().checked_sub(pattern.len())?;
        let hit = |i: &usize| self.chars[*i..*i + pattern.len()] == *pattern;
        if forward {
            (from + 1..=last).chain(0..=from.min(last)).find(hit)
        } else {
            (0..from.min(last + 1))
                .rev()
                .chain((from..=last).rev())
                .find(hit)
        }
    }
}

#[derive(Default)]
pub struct Vim {
    mode: ModeState,
    /// The count being typed. 0 means none.
    count: usize,
    /// A pending operator and the count typed before it.
    operator: Option<(char, usize)>,
    /// `g` was pressed and waits for a second `g`.
    g_pending: bool,
    /// Visual mode: where the selection started and where the cursor is, in characters.
    anchor: usize,
    head: usize,
    /// What `y`, `d`, `c` and `x` took, and whether it is whole lines.
    register: (String, bool),
    search: Option<Vec<char>>,
    /// A `/` search or a `:` command being typed, with its first character.
    command: Option<String>,
}

#[derive(Default)]
struct ModeState(Option<Mode>);

impl Vim {
    pub fn new() -> Vim {
        Vim::default()
    }

    pub fn mode(&self) -> Mode {
        self.mode.0.unwrap_or(Mode::Normal)
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode.0 = Some(mode);
    }

    /// The line to show under the editor: the mode, or the command being typed.
    pub fn status(&self) -> String {
        if let Some(command) = &self.command {
            return command.clone();
        }
        let mode = match self.mode() {
            Mode::Normal => "-- NORMAL --",
            Mode::Insert => "-- INSERT --",
            Mode::Visual => "-- VISUAL --",
            Mode::VisualLine => "-- VISUAL LINE --",
        };
        let mut pending = String::new();
        if let Some((op, count)) = self.operator {
            if count > 0 {
                pending.push_str(&count.to_string());
            }
            pending.push(op);
        }
        if self.count > 0 {
            pending.push_str(&self.count.to_string());
        }
        if self.g_pending {
            pending.push('g');
        }
        if pending.is_empty() {
            mode.to_string()
        } else {
            format!("{mode}  {pending}")
        }
    }

    /// Feed one key. Printable keys are the character itself (`"d"`, `"G"`, `"$"`, `" "`).
    /// The others are `"escape"`, `"enter"`, `"backspace"` and `"ctrl-r"`.
    pub fn handle(&mut self, key: &str, buf: &mut dyn VimBuffer) -> Handled {
        if self.command.is_some() {
            return self.command_key(key, buf);
        }
        match self.mode() {
            Mode::Insert => {
                if key != "escape" {
                    return Handled::No;
                }
                // Leaving insert mode steps back onto the last character typed.
                let (doc, cursor) = Doc::of(buf);
                let back = cursor.saturating_sub(1).max(doc.line_start(cursor));
                buf.set_cursor(doc.byte(back));
                self.set_mode(Mode::Normal);
                Handled::Yes
            }
            Mode::Normal => self.normal_key(key, buf),
            Mode::Visual | Mode::VisualLine => self.visual_key(key, buf),
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.operator = None;
        self.g_pending = false;
    }

    /// Take a digit into the count. `0` is a motion unless a count is already being typed.
    fn take_digit(&mut self, key: &str) -> bool {
        match key.parse::<usize>() {
            Ok(digit) if key.len() == 1 && (digit != 0 || self.count > 0) => {
                self.count = self
                    .count
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(100_000);
                true
            }
            _ => false,
        }
    }

    /// Where a motion goes from `pos`. `count` is `None` when no count was typed.
    fn motion(
        &mut self,
        key: &str,
        doc: &Doc,
        pos: usize,
        count: Option<usize>,
    ) -> Option<(usize, Kind)> {
        let n = count.unwrap_or(1);
        let (start, end) = (doc.line_start(pos), doc.line_end(pos));
        let column = pos - start;
        let in_line = |line: usize| (line + column).min(doc.line_end(line));
        if std::mem::take(&mut self.g_pending) {
            return (key == "g").then(|| (doc.line_number(count.unwrap_or(1)), Kind::Linewise));
        }
        Some(match key {
            "h" => (pos.saturating_sub(n).max(start), Kind::Exclusive),
            "l" => ((pos + n).min(end), Kind::Exclusive),
            "j" => (in_line(doc.line_offset(pos, true, n)), Kind::Linewise),
            "k" => (in_line(doc.line_offset(pos, false, n)), Kind::Linewise),
            "w" => (
                (0..n).fold(pos, |p, _| doc.word_forward(p)),
                Kind::Exclusive,
            ),
            "b" => ((0..n).fold(pos, |p, _| doc.word_back(p)), Kind::Exclusive),
            "e" => ((0..n).fold(pos, |p, _| doc.word_end(p)), Kind::Inclusive),
            "0" => (start, Kind::Exclusive),
            "^" => (doc.first_non_blank(pos), Kind::Exclusive),
            "$" => {
                let line = doc.line_offset(pos, true, n - 1);
                (
                    doc.line_end(line).saturating_sub(1).max(line),
                    Kind::Inclusive,
                )
            }
            "G" => {
                let line = match count {
                    Some(number) => doc.line_number(number),
                    None => doc.line_start(doc.len()),
                };
                (line, Kind::Linewise)
            }
            "g" => {
                self.g_pending = true;
                return None;
            }
            _ => return None,
        })
    }

    fn normal_key(&mut self, key: &str, buf: &mut dyn VimBuffer) -> Handled {
        if key == "escape" {
            self.reset();
            return Handled::Yes;
        }
        if self.take_digit(key) {
            return Handled::Yes;
        }
        let (doc, cursor) = Doc::of(buf);
        let cursor = doc.clamp(cursor);
        let typed = (self.count > 0).then_some(self.count);
        let was_g = self.g_pending;

        // An operator waiting for its motion.
        if let Some((op, before)) = self.operator {
            let count = match (before, typed) {
                (0, None) => None,
                (a, b) => Some(a.max(1) * b.unwrap_or(1)),
            };
            if key.starts_with(op) && !was_g {
                // dd, yy, cc: whole lines.
                let last = doc.line_offset(cursor, true, count.unwrap_or(1) - 1);
                self.reset();
                self.operate(
                    op,
                    cursor.min(last)..cursor.max(last),
                    Kind::Linewise,
                    &doc,
                    buf,
                );
                return Handled::Yes;
            }
            // `cw` on a word changes to the end of the word, like `ce`.
            let motion_key = if op == 'c' && key == "w" && doc.class(cursor) != 0 {
                "e"
            } else {
                key
            };
            match self.motion(motion_key, &doc, cursor, count) {
                Some((mut target, kind)) => {
                    if key == "w" && motion_key == "w" && target > doc.line_end(cursor) {
                        // `dw` on the last word of a line stops at the line's end.
                        target = doc.line_end(cursor);
                    }
                    self.reset();
                    self.operate(op, cursor.min(target)..cursor.max(target), kind, &doc, buf);
                }
                None if self.g_pending => self.count = 0,
                None => self.reset(),
            }
            return Handled::Yes;
        }

        if let Some((target, _)) = self.motion(key, &doc, cursor, typed) {
            self.count = 0;
            buf.set_cursor(doc.byte(doc.clamp(target)));
            return Handled::Yes;
        }
        if self.g_pending {
            // `g` keeps the count for the `g` that follows.
            return Handled::Yes;
        }
        let n = typed.unwrap_or(1);
        self.count = 0;
        let (start, end) = (doc.line_start(cursor), doc.line_end(cursor));
        let insert_at = |vim: &mut Vim, buf: &mut dyn VimBuffer, pos: usize| {
            buf.set_cursor(doc.byte(pos));
            vim.set_mode(Mode::Insert);
        };
        match key {
            "d" | "c" | "y" => {
                self.operator = Some((key.chars().next().unwrap(), typed.unwrap_or(0)))
            }
            "i" => insert_at(self, buf, cursor),
            "a" => insert_at(self, buf, (cursor + 1).min(end)),
            "I" => insert_at(self, buf, doc.first_non_blank(cursor)),
            "A" => insert_at(self, buf, end),
            "o" => {
                buf.replace(doc.bytes(end..end), "\n");
                buf.set_cursor(doc.byte(end) + 1);
                self.set_mode(Mode::Insert);
            }
            "O" => {
                buf.replace(doc.bytes(start..start), "\n");
                buf.set_cursor(doc.byte(start));
                self.set_mode(Mode::Insert);
            }
            "x" if end > start => {
                let range = cursor..(cursor + n).min(end);
                self.register = (doc.slice(range.clone()), false);
                buf.replace(doc.bytes(range), "");
                self.settle(buf, cursor);
            }
            "p" | "P" => self.paste(key == "P", n, cursor, &doc, buf),
            "u" => (0..n).for_each(|_| buf.undo()),
            "ctrl-r" => (0..n).for_each(|_| buf.redo()),
            "v" | "V" => {
                (self.anchor, self.head) = (cursor, cursor);
                self.set_mode(if key == "v" {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                });
                self.show_selection(buf);
            }
            "n" | "N" => {
                if let Some(pattern) = self.search.clone()
                    && let Some(hit) = doc.find(&pattern, cursor, key == "n")
                {
                    buf.set_cursor(doc.byte(hit));
                }
            }
            "/" | ":" => self.command = Some(key.to_string()),
            _ => {}
        }
        Handled::Yes
    }

    /// Put the cursor at `pos` in the text as it is now, on a character.
    fn settle(&self, buf: &mut dyn VimBuffer, pos: usize) {
        let (doc, _) = Doc::of(buf);
        buf.set_cursor(doc.byte(doc.clamp(pos.min(doc.len()))));
    }

    /// Run `d`, `c` or `y` over a range of characters, or over the lines it touches.
    fn operate(
        &mut self,
        op: char,
        range: Range<usize>,
        kind: Kind,
        doc: &Doc,
        buf: &mut dyn VimBuffer,
    ) {
        if kind == Kind::Linewise {
            let (first, last_end) = (doc.line_start(range.start), doc.line_end(range.end));
            self.register = (doc.slice(first..last_end) + "\n", true);
            match op {
                'y' => buf.set_cursor(doc.byte(first)),
                'c' => {
                    // The lines go, and one empty line stays to type on.
                    buf.replace(doc.bytes(first..last_end), "");
                    buf.set_cursor(doc.byte(first));
                    self.set_mode(Mode::Insert);
                }
                _ => {
                    // Take the line break too: the one after, or the one before the last line.
                    let cut = if last_end < doc.len() {
                        first..last_end + 1
                    } else {
                        first.saturating_sub(1)..last_end
                    };
                    let landing = cut.start + usize::from(cut.start > 0 && last_end >= doc.len());
                    buf.replace(doc.bytes(cut), "");
                    let (after, _) = Doc::of(buf);
                    let line = after.line_start(landing.min(after.len()));
                    buf.set_cursor(after.byte(after.first_non_blank(line)));
                }
            }
            return;
        }
        let end = if kind == Kind::Inclusive {
            range.end + 1
        } else {
            range.end
        };
        let range = range.start..end.min(doc.len());
        self.register = (doc.slice(range.clone()), false);
        match op {
            'y' => buf.set_cursor(doc.byte(range.start)),
            'c' => {
                buf.replace(doc.bytes(range.clone()), "");
                buf.set_cursor(doc.byte(range.start));
                self.set_mode(Mode::Insert);
            }
            _ => {
                buf.replace(doc.bytes(range.clone()), "");
                self.settle(buf, range.start);
            }
        }
    }

    fn paste(
        &mut self,
        before: bool,
        count: usize,
        cursor: usize,
        doc: &Doc,
        buf: &mut dyn VimBuffer,
    ) {
        let (text, linewise) = self.register.clone();
        if text.is_empty() {
            return;
        }
        let text = text.repeat(count);
        if linewise {
            let body = text.trim_end_matches('\n');
            if before {
                let start = doc.line_start(cursor);
                buf.replace(doc.bytes(start..start), &format!("{body}\n"));
                self.settle_first_non_blank(buf, start);
            } else {
                let end = doc.line_end(cursor);
                buf.replace(doc.bytes(end..end), &format!("\n{body}"));
                self.settle_first_non_blank(buf, end + 1);
            }
        } else {
            let empty = doc.line_start(cursor) == doc.line_end(cursor);
            let at = if before || empty { cursor } else { cursor + 1 };
            buf.replace(doc.bytes(at..at), &text);
            self.settle(buf, at + text.chars().count() - 1);
        }
    }

    fn settle_first_non_blank(&self, buf: &mut dyn VimBuffer, line: usize) {
        let (doc, _) = Doc::of(buf);
        buf.set_cursor(doc.byte(doc.first_non_blank(line.min(doc.len()))));
    }

    // ---- visual mode ------------------------------------------------------------------------

    /// The selected characters, or the selected lines without the last line break.
    fn visual_range(&self, doc: &Doc) -> Range<usize> {
        let (from, to) = (self.anchor.min(self.head), self.anchor.max(self.head));
        match self.mode() {
            Mode::VisualLine => doc.line_start(from)..doc.line_end(to),
            _ => from..(to + 1).min(doc.len()),
        }
    }

    fn show_selection(&self, buf: &mut dyn VimBuffer) {
        let (doc, _) = Doc::of(buf);
        buf.set_selection(doc.bytes(self.visual_range(&doc)));
    }

    fn visual_key(&mut self, key: &str, buf: &mut dyn VimBuffer) -> Handled {
        let (doc, _) = Doc::of(buf);
        self.head = self.head.min(doc.len());
        if key == "escape" {
            self.reset();
            self.set_mode(Mode::Normal);
            buf.set_cursor(doc.byte(doc.clamp(self.head)));
            return Handled::Yes;
        }
        if self.take_digit(key) {
            return Handled::Yes;
        }
        let typed = (self.count > 0).then_some(self.count);
        if let Some((target, _)) = self.motion(key, &doc, self.head, typed) {
            self.count = 0;
            self.head = doc.clamp(target);
            self.show_selection(buf);
            return Handled::Yes;
        }
        if self.g_pending {
            return Handled::Yes;
        }
        self.count = 0;
        let linewise = self.mode() == Mode::VisualLine;
        let range = self.visual_range(&doc);
        match key {
            "v" | "V" => {
                // The same key leaves visual mode. The other one switches kind.
                let wanted = if key == "v" {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                if self.mode() == wanted {
                    return self.visual_key("escape", buf);
                }
                self.set_mode(wanted);
                self.show_selection(buf);
            }
            "d" | "x" | "y" | "c" => {
                let op = if key == "x" {
                    'd'
                } else {
                    key.chars().next().unwrap()
                };
                self.set_mode(Mode::Normal);
                if linewise {
                    self.operate(op, range.start..range.end, Kind::Linewise, &doc, buf);
                } else {
                    self.operate(op, range, Kind::Exclusive, &doc, buf);
                }
            }
            ">" | "<" => {
                let lines = doc.line_start(range.start)
                    ..doc.line_end(range.end.saturating_sub(1).max(range.start));
                let shifted: Vec<String> = doc
                    .slice(lines.clone())
                    .split('\n')
                    .map(|line| match key {
                        ">" if line.is_empty() => String::new(),
                        ">" => format!("  {line}"),
                        _ => {
                            let cut = line.chars().take(2).take_while(|c| *c == ' ').count();
                            let cut = if line.starts_with('\t') { 1 } else { cut };
                            line.chars().skip(cut).collect()
                        }
                    })
                    .collect();
                buf.replace(doc.bytes(lines.clone()), &shifted.join("\n"));
                self.set_mode(Mode::Normal);
                self.settle_first_non_blank(buf, lines.start);
            }
            _ => {}
        }
        Handled::Yes
    }

    // ---- `/` and `:` ------------------------------------------------------------------------

    fn command_key(&mut self, key: &str, buf: &mut dyn VimBuffer) -> Handled {
        let mut command = self.command.take().unwrap_or_default();
        match key {
            "escape" => {}
            "backspace" => {
                command.pop();
                // Backspace over the `/` or `:` itself cancels.
                if !command.is_empty() {
                    self.command = Some(command);
                }
            }
            "enter" => {
                let body = &command[1..];
                if command.starts_with('/') {
                    if !body.is_empty() {
                        self.search = Some(body.chars().collect());
                    }
                    let (doc, cursor) = Doc::of(buf);
                    if let Some(pattern) = &self.search
                        && let Some(hit) = doc.find(pattern, cursor, true)
                    {
                        buf.set_cursor(doc.byte(hit));
                    }
                } else if matches!(body.trim(), "q" | "q!" | "wq" | "x") {
                    return Handled::Close;
                }
                // `:w` has nothing to do: cards save themselves.
            }
            _ if key.chars().count() == 1 => {
                command.push_str(key);
                self.command = Some(command);
            }
            _ => self.command = Some(command),
        }
        Handled::Yes
    }
}
