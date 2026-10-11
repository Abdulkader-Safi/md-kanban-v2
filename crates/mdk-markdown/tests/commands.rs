//! The cases from the Electron app's `md-commands.check.ts`, plus a few of mdk's own.

use mdk_markdown::commands::*;
use std::ops::Range;

/// Run a command on `marked`, where « and » mark the selection, or ¦ a cursor.
/// Returns the new text with the selection marked the same way.
fn run(command: impl Fn(&str, Range<usize>) -> Edit, marked: &str) -> String {
    let text: String = marked.chars().filter(|c| !"«»¦".contains(*c)).collect();
    let selection = match marked.find('¦') {
        Some(cursor) => cursor..cursor,
        None => {
            let from = marked.find('«').expect("a selection mark");
            from..marked.find('»').unwrap() - '«'.len_utf8()
        }
    };
    let Edit {
        mut text,
        selection,
    } = command(&text, selection);
    if selection.is_empty() {
        text.insert(selection.start, '¦');
    } else {
        text.insert(selection.end, '»');
        text.insert(selection.start, '«');
    }
    text
}

fn bold(text: &str, selection: Range<usize>) -> Edit {
    wrap(text, selection, "**", "**", "text")
}

fn list(kind: ListKind) -> impl Fn(&str, Range<usize>) -> Edit {
    move |text, selection| toggle_list(text, selection, kind)
}

fn head(level: usize) -> impl Fn(&str, Range<usize>) -> Edit {
    move |text, selection| heading(text, selection, level)
}

#[test]
fn wrap_toggles_marks() {
    assert_eq!(run(bold, "a «word» b"), "a **«word»** b");
    assert_eq!(
        run(bold, "a **«word»** b"),
        "a «word» b",
        "marks outside the selection come off"
    );
    assert_eq!(
        run(bold, "a «**word**» b"),
        "a «word» b",
        "marks inside the selection come off"
    );
    assert_eq!(
        run(bold, "a ¦ b"),
        "a **«text»** b",
        "empty selection gets a selected placeholder"
    );
    let italic = |t: &str, s: Range<usize>| wrap(t, s, "_", "_", "text");
    assert_eq!(
        run(italic, "**«bold»**"),
        "**_«bold»_**",
        "italic never eats half of a bold mark"
    );
    let highlight = |t: &str, s: Range<usize>| wrap(t, s, "==", "==", "text");
    assert_eq!(
        run(highlight, "é «ünï» é"),
        "é ==«ünï»== é",
        "byte offsets stay on characters"
    );
}

#[test]
fn links_and_images() {
    let link_ = |t: &str, s: Range<usize>| link(t, s, false);
    let image = |t: &str, s: Range<usize>| link(t, s, true);
    assert_eq!(run(link_, "see «docs» here"), "see [docs](«https://») here");
    assert_eq!(run(link_, "«https://x.dev»"), "[«text»](https://x.dev)");
    assert_eq!(run(image, "¦"), "![«alt»](image.png)");
    assert_eq!(run(image, "«./pic.png»"), "![«alt»](./pic.png)");
}

#[test]
fn headings() {
    assert_eq!(run(head(2), "Ti¦tle"), "## Ti¦tle");
    assert_eq!(
        run(head(2), "## Ti¦tle"),
        "Ti¦tle",
        "same level again removes it"
    );
    assert_eq!(
        run(head(1), "### Ti¦tle"),
        "# Ti¦tle",
        "another level is replaced"
    );
    assert_eq!(
        run(head(1), "¦"),
        "# ¦",
        "the cursor lands after the mark on an empty line"
    );
    assert_eq!(run(head(3), "«one\n\ntwo»"), "«### one\n\n### two»");
}

#[test]
fn lists_and_quotes() {
    assert_eq!(run(list(ListKind::Bullet), "«one\ntwo»"), "«- one\n- two»");
    assert_eq!(run(list(ListKind::Bullet), "«- one\n- two»"), "«one\ntwo»");
    assert_eq!(
        run(list(ListKind::Ordered), "«- one\n\n- two»"),
        "«1. one\n\n2. two»",
        "blank lines are skipped and numbering continues"
    );
    assert_eq!(
        run(list(ListKind::Task), "  - it¦em"),
        "  - [ ] it¦em",
        "indent kept, bullet swapped for a task"
    );
    assert_eq!(
        run(list(ListKind::Bullet), "- [ ] it¦em"),
        "- it¦em",
        "a task is not a bullet"
    );
    assert_eq!(run(list(ListKind::Quote), "sa¦id"), "> sa¦id");
    assert_eq!(run(list(ListKind::Quote), "> sa¦id"), "sa¦id");
    assert_eq!(run(list(ListKind::Task), "- [x] do¦ne"), "do¦ne");
}

#[test]
fn blocks() {
    assert_eq!(run(code_block, "«x = 1\ny = 2»"), "```¦\nx = 1\ny = 2\n```");
    assert_eq!(run(code_block, "para¦"), "para\n\n```\n¦\n```");
    assert_eq!(
        run(table, "¦"),
        "| «Column» | Column |\n| --- | --- |\n|  |  |"
    );
    assert_eq!(
        run(callout, "text¦\nnext"),
        "text\n\n> [!note] «Title»\n> Text\n\nnext"
    );
    assert_eq!(
        run(mermaid, "¦"),
        "```mermaid\nflowchart TD\n  «A[Start] --> B[Done]»\n```"
    );
    assert_eq!(
        run(rule, "a\n¦"),
        "a\n\n---¦",
        "an empty line after text gets a blank line first"
    );
    assert_eq!(run(math_block, "a¦\n\nb"), "a\n\n$$\n¦\n$$\n\nb");
}

#[test]
fn enter_continues_lists_checklists_and_quotes() {
    let enter = |marked: &str| run(|t, s| continue_list(t, s).expect("a list line"), marked);
    assert_eq!(enter("- one¦"), "- one\n- ¦");
    assert_eq!(enter("  * one¦\nnext"), "  * one\n  * ¦\nnext");
    assert_eq!(
        enter("- [x] done¦"),
        "- [x] done\n- [ ] ¦",
        "a new task starts unticked"
    );
    assert_eq!(enter("9. nine¦"), "9. nine\n10. ¦");
    assert_eq!(enter("1) one¦"), "1) one\n2) ¦");
    assert_eq!(enter("> said¦"), "> said\n> ¦");
    assert_eq!(
        enter("- one¦ two"),
        "- one\n- ¦ two",
        "the rest of the line moves down"
    );
    // Enter on an empty item ends the list.
    assert_eq!(enter("- one\n- ¦"), "- one\n¦");
    assert_eq!(enter("- [ ] ¦"), "¦");
    // Not a list line, a selection, or the cursor inside the mark: Enter is left alone.
    assert_eq!(continue_list("plain", 5..5), None);
    assert_eq!(continue_list("- one", 2..5), None);
    assert_eq!(continue_list("- one", 1..1), None);
    assert_eq!(continue_list("# heading", 9..9), None);
}
