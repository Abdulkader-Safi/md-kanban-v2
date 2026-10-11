//! The formatting toolbar's edits. Each command takes the text and the selection and
//! returns the new text and the new selection, so it runs and is tested without an editor.
//! Offsets are byte offsets into the text, on character boundaries.

use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;

/// The result of a command: the whole new text, and what to select in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub selection: Range<usize>,
}

/// One replacement in the original text.
struct Change {
    from: usize,
    to: usize,
    insert: String,
}

/// Apply changes that are sorted and do not overlap.
fn apply(text: &str, changes: &[Change]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for change in changes {
        out.push_str(&text[at..change.from]);
        out.push_str(&change.insert);
        at = change.to;
    }
    out.push_str(&text[at..]);
    out
}

/// Where a position in the old text ends up. `after` picks the side when text was inserted
/// right at the position: before the insertion, or after it.
fn map_pos(pos: usize, changes: &[Change], after: bool) -> usize {
    let mut mapped = pos as isize;
    for change in changes {
        if pos < change.from {
            break;
        }
        let (removed, added) = (
            (change.to - change.from) as isize,
            change.insert.len() as isize,
        );
        if pos > change.to || (pos == change.to && change.from < change.to) {
            mapped += added - removed;
        } else {
            // Inside the replaced text, or exactly at an insertion.
            mapped = mapped - (pos - change.from) as isize + if after { added } else { 0 };
        }
    }
    mapped as usize
}

/// The line holding `pos`, as a byte range without its line break.
fn line_at(text: &str, pos: usize) -> Range<usize> {
    let start = text[..pos].rfind('\n').map_or(0, |i| i + 1);
    let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
    start..end
}

/// The lines the selection touches. Blank ones are left out, unless they are all blank.
fn selected_lines(text: &str, selection: &Range<usize>) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut line = line_at(text, selection.start);
    loop {
        let end = line.end;
        lines.push(line);
        if end >= selection.end || end >= text.len() {
            break;
        }
        line = line_at(text, end + 1);
    }
    let content: Vec<_> = lines
        .iter()
        .filter(|l| !text[(*l).clone()].trim().is_empty())
        .cloned()
        .collect();
    if content.is_empty() { lines } else { content }
}

/// Changes at line starts. A cursor ends up after anything inserted where it stood; a
/// selection grows to take the new marks in.
fn line_changes(text: &str, selection: &Range<usize>, changes: Vec<Change>) -> Edit {
    let selection = if selection.is_empty() {
        let cursor = map_pos(selection.start, &changes, true);
        cursor..cursor
    } else {
        map_pos(selection.start, &changes, false)..map_pos(selection.end, &changes, true)
    };
    Edit {
        text: apply(text, &changes),
        selection,
    }
}

/// Wrap the selection in marks, or take the marks off when they are already there.
/// An empty selection gets a placeholder, selected so typing replaces it.
pub fn wrap(
    text: &str,
    selection: Range<usize>,
    before: &str,
    after: &str,
    placeholder: &str,
) -> Edit {
    let (from, to) = (selection.start, selection.end);
    let selected = &text[from..to];
    let one = |from: usize, to: usize, insert: String, selection: Range<usize>| Edit {
        text: apply(text, &[Change { from, to, insert }]),
        selection,
    };

    // Marks just outside the selection.
    if from >= before.len() && text[..from].ends_with(before) && text[to..].starts_with(after) {
        let start = from - before.len();
        return one(
            start,
            to + after.len(),
            selected.to_string(),
            start..start + selected.len(),
        );
    }
    // Marks just inside it.
    if selected.len() >= before.len() + after.len()
        && selected.starts_with(before)
        && selected.ends_with(after)
    {
        let inner = &selected[before.len()..selected.len() - after.len()];
        return one(from, to, inner.to_string(), from..from + inner.len());
    }
    let inner = if selected.is_empty() {
        placeholder
    } else {
        selected
    };
    let start = from + before.len();
    one(
        from,
        to,
        format!("{before}{inner}{after}"),
        start..start + inner.len(),
    )
}

/// `[label](url)` around the selection, or `![alt](image.png)`. A selected URL becomes the
/// target. What still needs typing is left selected.
pub fn link(text: &str, selection: Range<usize>, image: bool) -> Edit {
    static URL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(https?://|\.{0,2}/)\S*$").unwrap());
    let selected = &text[selection.clone()];
    let is_url = URL.is_match(selected);
    let bang = if image { "!" } else { "" };
    let fallback = if image { "alt" } else { "text" };
    let label = if is_url || selected.is_empty() {
        fallback
    } else {
        selected
    };
    let url = match (is_url, image) {
        (true, _) => selected,
        (false, true) => "image.png",
        (false, false) => "https://",
    };
    let label_at = selection.start + bang.len() + 1;
    let url_at = label_at + label.len() + 2;
    // The label for a pasted URL, the URL for selected words.
    let select = if !selected.is_empty() && !is_url {
        url_at..url_at + url.len()
    } else {
        label_at..label_at + label.len()
    };
    let insert = format!("{bang}[{label}]({url})");
    Edit {
        text: apply(
            text,
            &[Change {
                from: selection.start,
                to: selection.end,
                insert,
            }],
        ),
        selection: select,
    }
}

/// Make the selected lines a heading of `level`. The same level again removes it.
pub fn heading(text: &str, selection: Range<usize>, level: usize) -> Edit {
    static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s+").unwrap());
    let mark = format!("{} ", "#".repeat(level));
    let deeper = format!("{}#", mark.trim());
    let lines = selected_lines(text, &selection);
    let on = lines.iter().all(|l| {
        let line = &text[l.clone()];
        line.starts_with(&mark) && !line.starts_with(&deeper)
    });
    let changes = lines
        .iter()
        .map(|l| Change {
            from: l.start,
            to: l.start + HEADING.find(&text[l.clone()]).map_or(0, |m| m.end()),
            insert: if on { String::new() } else { mark.clone() },
        })
        .collect();
    line_changes(text, &selection, changes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered,
    Task,
    Quote,
}

/// Turn the selected lines into a list or a quote. Lines that already are that kind lose
/// it. Another kind of list is swapped, keeping the indent.
pub fn toggle_list(text: &str, selection: Range<usize>, kind: ListKind) -> Edit {
    static TASK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-*+] \[[ xX]\] ").unwrap());
    static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-*+] ").unwrap());
    static ORDERED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\d+[.)] ").unwrap());
    static QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*> ?").unwrap());
    static ANY_MARK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\s*)(?:[-*+] \[[ xX]\] |[-*+] |\d+[.)] |> ?)?").unwrap());
    let is_kind = |line: &str| match kind {
        ListKind::Task => TASK.is_match(line),
        // A task starts like a bullet, and is not one.
        ListKind::Bullet => BULLET.is_match(line) && !TASK.is_match(line),
        ListKind::Ordered => ORDERED.is_match(line),
        ListKind::Quote => QUOTE.is_match(line),
    };
    let lines = selected_lines(text, &selection);
    let on = lines.iter().all(|l| is_kind(&text[l.clone()]));
    let changes = lines
        .iter()
        .enumerate()
        .map(|(n, l)| {
            let found = ANY_MARK.captures(&text[l.clone()]).expect("always matches");
            let mark = match kind {
                ListKind::Ordered => format!("{}. ", n + 1),
                ListKind::Task => "- [ ] ".to_string(),
                ListKind::Bullet => "- ".to_string(),
                ListKind::Quote => "> ".to_string(),
            };
            Change {
                from: l.start,
                to: l.start + found[0].len(),
                insert: format!("{}{}", &found[1], if on { "" } else { mark.as_str() }),
            }
        })
        .collect();
    line_changes(text, &selection, changes)
}

/// Insert a block on its own lines below the cursor's line, or on that line when it is
/// empty, with blank lines around it. `select` is the range inside `block` to select.
pub fn block(text: &str, selection: Range<usize>, block: &str, select: Range<usize>) -> Edit {
    let line = line_at(text, selection.end);
    let blank = |range: Range<usize>| text[range].trim().is_empty();
    let empty = blank(line.clone());
    let pre = if !empty {
        "\n\n"
    } else if line.start > 0 && !blank(line_at(text, line.start - 1)) {
        "\n"
    } else {
        ""
    };
    let next_has_text = line.end < text.len() && !blank(line_at(text, line.end + 1));
    let post = if next_has_text { "\n" } else { "" };
    let from = if empty { line.start } else { line.end };
    let at = from + pre.len();
    Edit {
        text: apply(
            text,
            &[Change {
                from,
                to: line.end,
                insert: format!("{pre}{block}{post}"),
            }],
        ),
        selection: at + select.start..at + select.end,
    }
}

/// Fence the selected lines, or insert an empty fence.
pub fn code_block(text: &str, selection: Range<usize>) -> Edit {
    if selection.is_empty() {
        return block(text, selection, "```\n\n```", 4..4);
    }
    let first = line_at(text, selection.start);
    let last = line_at(text, selection.end);
    let changes = [
        Change {
            from: first.start,
            to: first.start,
            insert: "```\n".into(),
        },
        Change {
            from: last.end,
            to: last.end,
            insert: "\n```".into(),
        },
    ];
    Edit {
        text: apply(text, &changes),
        selection: first.start + 3..first.start + 3,
    }
}

pub fn table(text: &str, selection: Range<usize>) -> Edit {
    block(
        text,
        selection,
        "| Column | Column |\n| --- | --- |\n|  |  |",
        2..8,
    )
}

pub fn callout(text: &str, selection: Range<usize>) -> Edit {
    block(text, selection, "> [!note] Title\n> Text", 10..15)
}

pub fn math_block(text: &str, selection: Range<usize>) -> Edit {
    block(text, selection, "$$\n\n$$", 3..3)
}

pub fn mermaid(text: &str, selection: Range<usize>) -> Edit {
    block(
        text,
        selection,
        "```mermaid\nflowchart TD\n  A[Start] --> B[Done]\n```",
        26..46,
    )
}

/// A horizontal rule.
pub fn rule(text: &str, selection: Range<usize>) -> Edit {
    block(text, selection, "---", 3..3)
}

/// Enter at the end of a list item, checklist item or quote line: start the next one.
/// On an item with nothing in it, Enter ends the list by removing the mark. Returns
/// `None` when the cursor is not in such a line, so Enter does what it normally does.
pub fn continue_list(text: &str, selection: Range<usize>) -> Option<Edit> {
    static ITEM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(\s*)(?:([-*+]) (\[[ xX]\] )?|(\d+)([.)]) |(>) ?)").unwrap()
    });
    if !selection.is_empty() {
        return None;
    }
    let cursor = selection.start;
    let line = line_at(text, cursor);
    let found = ITEM.captures(&text[line.clone()])?;
    let mark_len = found[0].len();
    if cursor < line.start + mark_len {
        return None;
    }
    if text[line.start + mark_len..line.end].trim().is_empty() {
        // An empty item: take the mark away and leave a plain empty line.
        let change = Change {
            from: line.start,
            to: line.end,
            insert: String::new(),
        };
        return Some(Edit {
            text: apply(text, &[change]),
            selection: line.start..line.start,
        });
    }
    let indent = &found[1];
    let mark = if let Some(bullet) = found.get(2) {
        // A new checklist item starts unticked.
        let task = if found.get(3).is_some() { "[ ] " } else { "" };
        format!("{} {task}", bullet.as_str())
    } else if let Some(number) = found.get(4) {
        let next = number.as_str().parse::<u64>().map_or(1, |n| n + 1);
        format!("{next}{} ", &found[5])
    } else {
        "> ".to_string()
    };
    let insert = format!("\n{indent}{mark}");
    let at = cursor + insert.len();
    let change = Change {
        from: cursor,
        to: cursor,
        insert,
    };
    Some(Edit {
        text: apply(text, &[change]),
        selection: at..at,
    })
}
