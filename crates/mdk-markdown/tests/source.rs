//! The preview's text rules. Where the Electron app's `render.check.ts` has a case that
//! does not depend on HTML output, it is here with the same input.

use mdk_markdown::source::*;

fn cards() -> Vec<CardRef> {
    let card = |id: &str, title: &str, rel_path: &str| CardRef {
        id: id.into(),
        title: title.into(),
        rel_path: rel_path.into(),
    };
    vec![
        card(
            "login-2026-09-06",
            "Rebuild login form",
            "auth/login-2026-09-06.md",
        ),
        card(
            "tokens-2026-09-07",
            "Token refresh",
            "auth/tokens-2026-09-07.md",
        ),
        card("setup-2026-09-01", "Project setup", "setup-2026-09-01.md"),
    ]
}

const PAGE: &str = "auth/login-2026-09-06.md";

fn p(markdown: &str) -> String {
    prepare(markdown, &cards()).source
}

#[test]
fn relative_links_find_cards_inside_the_project_only() {
    let cards = cards();
    let found = |href: &str| find_link(&cards, PAGE, href).map(|c| c.id.as_str());
    assert_eq!(found("./tokens-2026-09-07.md"), Some("tokens-2026-09-07"));
    assert_eq!(found("../setup-2026-09-01"), Some("setup-2026-09-01"));
    assert_eq!(
        found("/setup-2026-09-01.md"),
        Some("setup-2026-09-01"),
        "a leading / is the project root"
    );
    assert_eq!(
        found("tokens-2026-09-07.md#Steps"),
        Some("tokens-2026-09-07")
    );
    assert_eq!(
        found("../../secret.md"),
        None,
        "a link that climbs out goes nowhere"
    );
    assert_eq!(found("nope.md"), None);
    assert_eq!(found("100%.md"), None, "a stray % must not crash");
}

#[test]
fn image_paths_resolve_against_the_card_and_never_leave_the_project() {
    assert_eq!(
        resolve_path(PAGE, "./img/diagram.png").as_deref(),
        Some("auth/img/diagram.png")
    );
    assert_eq!(
        resolve_path(PAGE, "img/a%20b.png").as_deref(),
        Some("auth/img/a b.png")
    );
    assert_eq!(resolve_path(PAGE, "/top.png").as_deref(), Some("top.png"));
    assert_eq!(resolve_path(PAGE, "../../../etc/passwd.png"), None);
    assert_eq!(resolve_path("root.md", "../x.png"), None);
    assert_eq!(
        resolve_path(PAGE, "50%.png").as_deref(),
        Some("auth/50%.png")
    );
    assert!(is_image("a/B.PNG") && is_image("x.svg") && !is_image("notes.md") && !is_image("png"));
    assert!(
        is_external("https://x.dev")
            && is_external("mailto:a@b.c")
            && !is_external("javascript:alert(1)")
    );
}

#[test]
fn wikilinks_become_card_links() {
    assert_eq!(
        p("see [[Token refresh]]"),
        "see [Token refresh](mdk-card:auth/tokens-2026-09-07.md)",
        "by title"
    );
    assert_eq!(
        p("see [[tokens-2026-09-07|the refresh card]]"),
        "see [the refresh card](mdk-card:auth/tokens-2026-09-07.md)",
        "by id, with a label"
    );
    assert_eq!(
        p("see [[project setup#Steps]]"),
        "see [project setup > Steps](mdk-card:setup-2026-09-01.md)",
        "ignoring case, with a heading"
    );
    assert_eq!(
        p("[[auth/login-2026-09-06]]"),
        "[auth/login-2026-09-06](mdk-card:auth/login-2026-09-06.md)",
        "by path"
    );
    assert_eq!(
        p("try [[not a card]] now"),
        "try [not a card](mdk-missing:) now"
    );
    assert_eq!(
        p("`[[Token refresh]]`"),
        "`[[Token refresh]]`",
        "never inside code"
    );
    assert_eq!(
        p("![[Token refresh]]"),
        "[Token refresh](mdk-card:auth/tokens-2026-09-07.md)",
        "a card embed links to the card"
    );
    assert_eq!(p("a [[x|y [z]]] b"), "a [y \\[z](mdk-missing:)] b");
}

#[test]
fn image_embeds_become_images_with_their_size_in_the_title() {
    assert_eq!(
        p("![[Screen Shot.png]]"),
        "![Screen Shot.png](Screen%20Shot.png \"\")"
    );
    assert_eq!(
        p("![[diagram.png|300x200]]"),
        "![diagram.png](diagram.png \"300x200\")"
    );
    assert_eq!(p("![[pic.PNG|300]]"), "![pic.PNG](pic.PNG \"300\")");
}

#[test]
fn highlights() {
    assert_eq!(
        p("This is ==important== text."),
        "This is [important](mdk-hl:) text."
    );
    assert_eq!(
        p("if a == b and c == d"),
        "if a == b and c == d",
        "spaces inside mean no highlight"
    );
    assert_eq!(p("`x ==y== z`"), "`x ==y== z`");
    assert_eq!(p("$a==b==c$"), "$a==b==c$", "never inside maths");
}

#[test]
fn fenced_code_maths_and_diagrams_are_left_exactly_as_written() {
    let mermaid = "```mermaid\ngraph TD\n  A[[Sub]] ==> B{Ok?}\n  B -->|<b>yes</b>| C\n```\n";
    assert_eq!(
        p(mermaid),
        mermaid,
        "no wikilinks, highlights or escapes inside a diagram"
    );
    let code = "~~~html\n<script>alert(1)</script> [[x]] ==y==\n~~~\nafter ==z==\n";
    assert_eq!(
        p(code),
        "~~~html\n<script>alert(1)</script> [[x]] ==y==\n~~~\nafter [z](mdk-hl:)\n"
    );
    let math = "$$\n[[a]] ==b== <c>\n$$\n==d==\n";
    assert_eq!(p(math), "$$\n[[a]] ==b== <c>\n$$\n[d](mdk-hl:)\n");
    assert_eq!(
        p("$$x==y==z$$\n==w=="),
        "$$x==y==z$$\n[w](mdk-hl:)",
        "a one-line block closes itself"
    );
}

#[test]
fn raw_html_is_shown_as_text_except_line_breaks() {
    for evil in [
        "<script>alert(1)</script>",
        "<img src=\"x\" onerror=\"alert(1)\">",
        "<a href=\"javascript:alert(1)\">x</a>",
        "<iframe src=\"https://evil.test\"></iframe>",
        "<div onclick=\"alert(1)\">x</div>",
        "<style>body{display:none}</style>",
        "<!-- comment -->",
    ] {
        let out = p(evil);
        assert!(
            !out.contains('<') || out.matches('<').count() == out.matches("\\<").count(),
            "{evil} -> {out}"
        );
    }
    assert_eq!(
        p("one<br>two<br/>three<BR />four"),
        "one<br>two<br/>three<BR />four"
    );
    assert_eq!(
        p("see <https://x.dev> and <mailto:a@b.c>"),
        "see <https://x.dev> and <mailto:a@b.c>"
    );
    assert_eq!(p("1 < 2 and a <b"), "1 < 2 and a \\<b");
    assert_eq!(
        p("already \\<b>"),
        "already \\<b>",
        "an escape is not escaped twice"
    );
}

#[test]
fn code_fence_titles_are_set_aside() {
    let prepared = prepare(
        "```ts title=\"lib/auth.ts\" {2}\nconst a = 1\nconst b = 2\n```\n",
        &[],
    );
    assert_eq!(prepared.source, "```ts\nconst a = 1\nconst b = 2\n```\n");
    assert_eq!(
        prepared
            .code_titles
            .get(&code_key("const a = 1\nconst b = 2"))
            .map(String::as_str),
        Some("lib/auth.ts")
    );
    let plain = prepare("```bash\ncurl https://x.dev\n```", &[]);
    assert_eq!(plain.source, "```bash\ncurl https://x.dev\n```");
    assert!(plain.code_titles.is_empty());
    let no_lang = prepare("```title=\"x.txt\"\nhi\n```", &[]);
    assert_eq!(no_lang.source, "```\nhi\n```");
    assert_eq!(no_lang.code_titles[&code_key("hi")], "x.txt");
}

#[test]
fn callouts() {
    let note = parse_callout("> [!NOTE]\n> Tokens are scoped to one project.").unwrap();
    assert_eq!(
        (note.tone, note.label.as_str(), note.folded),
        (Tone::Note, "NOTE", None)
    );
    assert_eq!(note.body, "Tokens are scoped to one project.");
    assert_eq!(
        parse_callout("> [!bug]\n> it breaks").unwrap().tone,
        Tone::Danger
    );
    assert_eq!(
        parse_callout("> [!whatever]\n> x").unwrap().tone,
        Tone::Note,
        "unknown type falls back to a note"
    );
    let titled = parse_callout("> [!tip] Before you start\n> Install bun.").unwrap();
    assert_eq!(
        (titled.tone, titled.label.as_str(), titled.body.as_str()),
        (Tone::Tip, "Before you start", "Install bun.")
    );
    let folded = parse_callout("> [!faq]- Why?\n> Because.").unwrap();
    assert_eq!(
        (folded.tone, folded.label.as_str(), folded.folded),
        (Tone::Warning, "Why?", Some(true))
    );
    assert_eq!(
        parse_callout("> [!faq]+ Open\n> x").unwrap().folded,
        Some(false)
    );
    for (kind, tone) in [
        ("IMPORTANT", Tone::Note),
        ("WARNING", Tone::Warning),
        ("CAUTION", Tone::Warning),
        ("quote", Tone::Quote),
    ] {
        assert_eq!(
            parse_callout(&format!("> [!{kind}]\n> x")).unwrap().tone,
            tone
        );
    }
    assert_eq!(parse_callout("> just a quote"), None);
    assert_eq!(parse_callout("> [!not a type] x"), None);
    assert_eq!(
        parse_callout(">[!note]\n>tight\n>\n> two").unwrap().body,
        "tight\n\ntwo"
    );
}

#[test]
fn the_dollar_guard_keeps_prices_as_text() {
    assert!(dollar_guard("E = mc^2", Some(' ')));
    assert!(dollar_guard("x", None));
    // "The plan costs $5 a month, or $50 a year." parses as maths "5 a month, or " then "50".
    assert!(!dollar_guard("5 a month, or ", Some('5')));
    assert!(!dollar_guard(" x", None) && !dollar_guard("x ", None) && !dollar_guard("", None));
}
