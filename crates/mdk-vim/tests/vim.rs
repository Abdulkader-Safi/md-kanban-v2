//! Every command in the FR-VIM-02 list, as (text with ¦ for the cursor, keys, result).

use mdk_vim::{Handled, Mode, Vim, VimBuffer};
use std::ops::Range;

/// A plain string with a cursor and an undo history: the simplest possible buffer.
#[derive(Default)]
struct Buf {
    text: String,
    cursor: usize,
    selection: Option<Range<usize>>,
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
}

impl VimBuffer for Buf {
    fn text(&self) -> String {
        self.text.clone()
    }
    fn cursor(&self) -> usize {
        self.cursor
    }
    fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos;
        self.selection = None;
    }
    fn set_selection(&mut self, range: Range<usize>) {
        self.selection = Some(range);
    }
    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.undo.push((self.text.clone(), self.cursor));
        self.redo.clear();
        self.text.replace_range(range.clone(), text);
        self.cursor = range.start + text.len();
    }
    fn undo(&mut self) {
        if let Some((text, cursor)) = self.undo.pop() {
            self.redo
                .push((std::mem::replace(&mut self.text, text), self.cursor));
            self.cursor = cursor;
        }
    }
    fn redo(&mut self) {
        if let Some((text, cursor)) = self.redo.pop() {
            self.undo
                .push((std::mem::replace(&mut self.text, text), self.cursor));
            self.cursor = cursor;
        }
    }
}

/// Split "d2w<esc>ihi<cr>" into keys. `<esc>`, `<cr>`, `<bs>` and `<c-r>` are special.
fn keys(script: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = script;
    while let Some(ch) = rest.chars().next() {
        let special = [
            ("<esc>", "escape"),
            ("<cr>", "enter"),
            ("<bs>", "backspace"),
            ("<c-r>", "ctrl-r"),
        ]
        .into_iter()
        .find(|(tag, _)| rest.starts_with(tag));
        match special {
            Some((tag, name)) => {
                out.push(name.to_string());
                rest = &rest[tag.len()..];
            }
            None => {
                out.push(ch.to_string());
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    out
}

struct Run {
    text: String,
    mode: Mode,
    selection: Option<String>,
    closed: bool,
    status: String,
}

/// Type `script` into `marked` and return the text with ¦ at the cursor.
fn run_full(marked: &str, script: &str) -> Run {
    let cursor = marked.find('¦').expect("a ¦ cursor");
    let mut buf = Buf {
        text: marked.replace('¦', ""),
        cursor,
        ..Default::default()
    };
    let mut vim = Vim::new();
    let mut closed = false;
    for key in keys(script) {
        match vim.handle(&key, &mut buf) {
            Handled::Yes => {}
            Handled::Close => closed = true,
            // Insert mode: the "editor" types the key.
            Handled::No => match key.as_str() {
                "enter" => buf.replace(buf.cursor..buf.cursor, "\n"),
                "backspace" => {
                    let before = buf.text[..buf.cursor]
                        .chars()
                        .next_back()
                        .map_or(0, char::len_utf8);
                    buf.replace(buf.cursor - before..buf.cursor, "");
                }
                typed => buf.replace(buf.cursor..buf.cursor, typed),
            },
        }
    }
    let selection = buf
        .selection
        .clone()
        .map(|range| buf.text[range].to_string());
    let mut text = buf.text.clone();
    text.insert(buf.cursor, '¦');
    Run {
        text,
        mode: vim.mode(),
        selection,
        closed,
        status: vim.status(),
    }
}

fn run(marked: &str, script: &str) -> String {
    run_full(marked, script).text
}

#[track_caller]
fn check(cases: &[(&str, &str, &str)]) {
    for (start, script, expected) in cases {
        assert_eq!(run(start, script), *expected, "{start:?} + {script:?}");
    }
}

#[test]
fn motions() {
    check(&[
        ("a¦bc", "h", "¦abc"),
        ("¦abc", "h", "¦abc"),
        ("¦abc", "l", "a¦bc"),
        ("ab¦c", "l", "ab¦c"),
        ("ab¦c\ndef", "j", "abc\nde¦f"),
        ("abc\nde¦f", "k", "ab¦c\ndef"),
        ("abc¦d\nx\nabcd", "j", "abcd\n¦x\nabcd"),
        ("¦one\ntwo", "k", "¦one\ntwo"),
        ("¦one two", "w", "one ¦two"),
        ("¦one, two", "w", "one¦, two"),
        ("on¦e\n  two", "w", "one\n  ¦two"),
        ("one ¦two", "b", "¦one two"),
        ("one tw¦o", "b", "one ¦two"),
        ("¦one two", "e", "on¦e two"),
        ("on¦e two", "e", "one tw¦o"),
        ("  one tw¦o", "0", "¦  one two"),
        ("  one tw¦o", "^", "  ¦one two"),
        ("¦one two\nx", "$", "one tw¦o\nx"),
        ("one\ntwo\nth¦ree", "gg", "¦one\ntwo\nthree"),
        ("o¦ne\ntwo\nthree", "G", "one\ntwo\n¦three"),
        ("o¦ne\ntwo\nthree", "2G", "one\n¦two\nthree"),
        ("one\ntwo\nth¦ree", "2gg", "one\n¦two\nthree"),
    ]);
}

#[test]
fn counts() {
    check(&[
        ("¦a\nb\nc\nd\ne", "3j", "a\nb\nc\n¦d\ne"),
        ("a\nb\nc\nd\n¦e", "2k", "a\nb\n¦c\nd\ne"),
        ("¦one two three four", "2w", "one two ¦three four"),
        ("¦abcdef", "3l", "abc¦def"),
        ("abcde¦f", "10h", "¦abcdef"),
        ("¦abcdefghijklm", "12l", "abcdefghijkl¦m"),
        ("one two thre¦e", "2b", "one ¦two three"),
    ]);
}

#[test]
fn inserting() {
    check(&[
        ("a¦bc", "iX<esc>", "a¦Xbc"),
        ("a¦bc", "aX<esc>", "ab¦Xc"),
        ("  a¦bc", "IX<esc>", "  ¦Xabc"),
        ("a¦bc", "AX<esc>", "abc¦X"),
        ("a¦bc\nz", "onew<esc>", "abc\nne¦w\nz"),
        ("z\na¦bc", "Onew<esc>", "z\nne¦w\nabc"),
        ("¦", "ihello<cr>world<esc>", "hello\nworl¦d"),
        ("ab¦c", "ix<bs>y<esc>", "ab¦yc"),
        // Keys typed in insert mode are text, not commands.
        ("¦", "idd yy p<esc>", "dd yy ¦p"),
    ]);
    assert_eq!(run_full("¦x", "i").mode, Mode::Insert);
    assert_eq!(run_full("¦x", "i<esc>").mode, Mode::Normal);
}

#[test]
fn x_dd_yy_p() {
    check(&[
        ("a¦bc", "x", "a¦c"),
        ("ab¦c", "x", "a¦b"),
        ("¦abcdef", "3x", "¦def"),
        ("a¦b", "5x", "¦a"),
        ("¦", "x", "¦"),
        ("one\nt¦wo\nthree", "dd", "one\n¦three"),
        ("one\ntwo\nth¦ree", "dd", "one\n¦two"),
        ("on¦e", "dd", "¦"),
        ("¦a\nb\nc\nd", "2dd", "¦c\nd"),
        ("a\n¦b\nc\nd", "d2d", "a\n¦d"),
        ("o¦ne\ntwo", "yyp", "one\n¦one\ntwo"),
        ("one\nt¦wo", "yyP", "one\n¦two\ntwo"),
        ("o¦ne\ntwo", "yy2p", "one\n¦one\none\ntwo"),
        ("one\n  t¦wo\nthree", "ddp", "one\nthree\n  ¦two"),
        ("¦abc", "xp", "b¦ac"),
        ("a¦bc", "xP", "a¦bc"),
        ("¦ab cd", "dwP", "ab¦ cd"),
    ]);
}

#[test]
fn undo_and_redo() {
    check(&[
        ("one\nt¦wo", "ddu", "one\nt¦wo"),
        ("one\nt¦wo", "ddu<c-r>", "¦one"),
        ("¦abc", "xxuu", "¦abc"),
        ("¦abc", "xx2u", "¦abc"),
        ("¦abc", "xxuu<c-r>", "¦bc"),
        ("¦abc", "u", "¦abc"),
    ]);
}

#[test]
fn operators_with_motions() {
    check(&[
        ("¦one two three", "dw", "¦two three"),
        ("one ¦two", "dw", "one¦ "),
        ("one ¦two\nthree", "dw", "one¦ \nthree"),
        ("¦one two three", "d2w", "¦three"),
        ("¦one two three", "2dw", "¦three"),
        ("one ¦two three", "db", "¦two three"),
        ("¦one two", "de", "¦ two"),
        ("one ¦two three", "d$", "one¦ "),
        ("  one ¦two", "d0", "¦two"),
        ("  one ¦two", "d^", "  ¦two"),
        ("a¦bcd", "dl", "a¦cd"),
        ("ab¦cd", "dh", "a¦cd"),
        ("a\n¦b\nc\nd", "dj", "a\n¦d"),
        ("a\nb\n¦c\nd", "dk", "a\n¦d"),
        ("a\nb\n¦c\nd", "dgg", "¦d"),
        ("a\n¦b\nc\nd", "dG", "¦a"),
        ("¦one two three", "cwX<esc>", "¦X two three"),
        ("one ¦two three", "c$X<esc>", "one ¦X"),
        ("one\nt¦wo\nthree", "ccX<esc>", "one\n¦X\nthree"),
        ("¦one two", "cwX<esc>", "¦X two"),
        ("¦one two three", "ywP", "one¦ one two three"),
        ("¦one two", "y$P", "one tw¦oone two"),
        ("¦a\nb\nc\nd", "y2jGp", "a\nb\nc\nd\n¦a\nb\nc"),
        ("a\n¦b", "ykP", "¦a\nb\na\nb"),
        // An operator and Escape, or an operator and a key that is no motion, do nothing.
        ("a¦bc", "d<esc>x", "a¦c"),
        ("a¦bc", "dz", "a¦bc"),
    ]);
    assert_eq!(run_full("¦one two", "cw").mode, Mode::Insert);
}

#[test]
fn visual_mode() {
    check(&[
        ("¦one two", "vld", "¦e two"),
        ("¦one two", "ved", "¦ two"),
        ("one ¦two three", "vwd", "one ¦hree"),
        ("on¦e two", "vhhd", "¦ two"),
        ("¦one two", "veyP", "on¦eone two"),
        ("¦one two", "vecX<esc>", "¦X two"),
        ("¦one two", "vex", "¦ two"),
        ("one\nt¦wo\nthree\nfour", "Vjd", "one\n¦four"),
        ("one\nt¦wo\nthree", "Vyp", "one\ntwo\n¦two\nthree"),
        ("one\nt¦wo\nthree", "VcX<esc>", "one\n¦X\nthree"),
        ("o¦ne\ntwo\nthree", "Vj>", "  ¦one\n  two\nthree"),
        ("  o¦ne\n    two\nthree", "Vj<", "¦one\n  two\nthree"),
        ("o¦ne\ntwo", "v>", "  ¦one\ntwo"),
        ("\to¦ne", "V<", "¦one"),
        ("¦one two", "vl<esc>x", "o¦e two"),
        ("¦a\nb\nc", "vjjd", "¦"),
    ]);
    let run = run_full("¦one two three", "vee");
    assert_eq!(
        (run.mode, run.selection.as_deref()),
        (Mode::Visual, Some("one two"))
    );
    let run = run_full("one\nt¦wo\nthree", "Vj");
    assert_eq!(
        (run.mode, run.selection.as_deref()),
        (Mode::VisualLine, Some("two\nthree"))
    );
    assert_eq!(run_full("¦one", "vv").mode, Mode::Normal);
    assert_eq!(run_full("¦one", "vV").mode, Mode::VisualLine);
    assert_eq!(run_full("¦one", "v<esc>").mode, Mode::Normal);
}

#[test]
fn searching() {
    check(&[
        ("¦one two one two", "/two<cr>", "one ¦two one two"),
        ("¦one two one two", "/two<cr>n", "one two one ¦two"),
        ("¦one two one two", "/two<cr>nn", "one ¦two one two"),
        ("one two one ¦two", "/two<cr>", "one ¦two one two"),
        ("¦one two one two", "/two<cr>nN", "one ¦two one two"),
        ("one two ¦one two", "/one<cr>N", "one two ¦one two"),
        ("¦abc", "/zzz<cr>", "¦abc"),
        ("¦abc abc", "/ab<bs>bc<cr>", "abc ¦abc"),
        ("¦one two", "/two<esc>x", "¦ne two"),
        ("¦a b a", "n", "¦a b a"),
        ("¦a.b a.b", "/a.b<cr>", "a.b ¦a.b"),
    ]);
    assert_eq!(run_full("¦abc", "/ab").status, "/ab");
}

#[test]
fn ex_commands() {
    // :w does nothing, because cards save themselves. :q closes the card.
    let saved = run_full("a¦bc", ":w<cr>x");
    assert_eq!((saved.text.as_str(), saved.closed), ("a¦c", false));
    assert!(run_full("¦abc", ":q<cr>").closed);
    assert!(run_full("¦abc", ":wq<cr>").closed);
    assert!(!run_full("¦abc", ":q<esc>").closed);
    assert!(!run_full("¦abc", ":nope<cr>").closed);
    assert!(!run_full("¦abc", ":<bs>x").closed);
    assert_eq!(run("¦abc", ":<bs>x"), "¦bc");
}

#[test]
fn status_line() {
    assert_eq!(run_full("¦x", "").status, "-- NORMAL --");
    assert_eq!(run_full("¦x", "i").status, "-- INSERT --");
    assert_eq!(run_full("¦x", "v").status, "-- VISUAL --");
    assert_eq!(run_full("¦x", "V").status, "-- VISUAL LINE --");
    assert_eq!(run_full("¦x", "2d3").status, "-- NORMAL --  2d3");
    assert_eq!(run_full("¦x", ":w").status, ":w");
}

#[test]
fn text_with_wide_characters() {
    check(&[
        ("¦é日本 ünï", "w", "é日本 ¦ünï"),
        ("é¦日本", "x", "é¦本"),
        ("¦é日本 ünï", "dw", "¦ünï"),
        ("日¦本\nünï", "ddp", "ünï\n¦日本"),
        ("¦日本", "vld", "¦"),
    ]);
}
