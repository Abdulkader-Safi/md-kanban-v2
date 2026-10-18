use mdk_format::*;

const NOW: &str = "2026-10-11T09:00:00.000Z";
const SUNDAY: Date = Date {
    year: 2026,
    month: 10,
    day: 11,
};

const CARD: &str = "---\n# written by an agent\nid: \"fix-login\"\nstatus: \"todo\"\nsprint: 4\nagent: \"claude\"   # who took it\nmeta:\n  nested: true\n  list:\n    - a\npriority: \"low\"\ncreated: \"2026-10-01T08:00:00Z\"\nlabels: [\"auth\"]\norder: 1\n---\n\n# Fix login\n\nThe agent's notes.\n";

#[test]
fn unknown_keys_survive_a_move_verbatim_after_the_known_keys() {
    let patch = CardPatch {
        status: Some(Status::Done),
        order: Some(7.5),
        ..Default::default()
    };
    let out = apply_patch(CARD, "fix-login.md", &patch, NOW).unwrap();
    assert_eq!(
        out,
        "---\nid: \"fix-login\"\nstatus: \"done\"\npriority: \"low\"\nassignee: \"\"\ndueDate: null\n\
         created: \"2026-10-01T08:00:00Z\"\nmodified: \"2026-10-11T09:00:00.000Z\"\n\
         labels: [\"auth\"]\norder: 7.5\n# written by an agent\nsprint: 4\n\
         agent: \"claude\"   # who took it\nmeta:\n  nested: true\n  list:\n    - a\n---\n\
         # Fix login\n\nThe agent's notes.\n"
    );
    // A second move changes nothing but the field it names. The comment survives again.
    let patch = CardPatch {
        status: Some(Status::Review),
        ..Default::default()
    };
    let again = apply_patch(&out, "fix-login.md", &patch, NOW).unwrap();
    assert_eq!(again, out.replace("status: \"done\"", "status: \"review\""));
}

#[test]
fn a_patch_keeps_what_an_agent_changed_on_disk() {
    // The agent rewrote the body after mdk last read the file. The move must keep it.
    let on_disk = CARD.replace(
        "The agent's notes.",
        "Rewritten by the agent.\n\n- [x] done",
    );
    let patch = CardPatch {
        status: Some(Status::InProgress),
        ..Default::default()
    };
    let out = apply_patch(&on_disk, "fix-login.md", &patch, NOW).unwrap();
    assert!(out.contains("status: \"in-progress\""));
    assert!(out.ends_with("# Fix login\n\nRewritten by the agent.\n\n- [x] done\n"));
}

#[test]
fn title_change_rewrites_the_first_heading_only() {
    let raw = "---\nid: \"a\"\n---\n# Old $1 title\n\n# Second heading\n";
    let patch = CardPatch {
        title: Some("New $0 title".into()),
        ..Default::default()
    };
    let out = apply_patch(raw, "a.md", &patch, NOW).unwrap();
    assert!(out.ends_with("---\n# New $0 title\n\n# Second heading\n"));
    assert_eq!(parse(&out, "a.md").title, "New $0 title");
}

#[test]
fn broken_yaml_is_shown_but_never_rewritten() {
    let raw = "---\nid: \"broken\"\nstatus: [todo\npriority: \"high\"\n---\n\n# Broken YAML\n";
    let card = parse(raw, "broken.md");
    assert!(card.parse_error.is_some());
    assert_eq!(
        (card.title.as_str(), card.priority),
        ("Broken YAML", Priority::High)
    );
    let patch = CardPatch {
        status: Some(Status::Done),
        ..Default::default()
    };
    assert!(matches!(
        apply_patch(raw, "broken.md", &patch, NOW),
        Err(FormatError::BrokenFrontmatter(_))
    ));
    // Valid YAML, a file with no frontmatter and an empty frontmatter are all fine.
    assert_eq!(parse(CARD, "c.md").parse_error, None);
    assert_eq!(parse("# Just a note\n", "n.md").parse_error, None);
    assert_eq!(parse("---\n---\n# Empty\n", "e.md").parse_error, None);
}

#[test]
fn crlf_and_bom_files_parse_and_stay_crlf() {
    let raw = "\u{feff}---\r\nid: \"win\"\r\nstatus: \"review\"\r\nlabels: [\"a\"]\r\norder: 3\r\n---\r\n\r\n# Windows card\r\n\r\nline two\r\n";
    let card = parse(raw, "win.md");
    assert_eq!(
        (card.id.as_str(), card.status, card.order),
        ("win", Status::Review, 3.0)
    );
    assert_eq!(card.body, "# Windows card\n\nline two");
    assert_eq!(card.line_ending, LineEnding::CrLf);
    let out = write(&card, NOW);
    assert!(out.ends_with("---\r\n# Windows card\r\n\r\nline two\r\n"));
    assert!(
        !out.replace("\r\n", "").contains(['\r', '\n']),
        "mixed line endings"
    );
}

#[test]
fn strings_are_json_quoted_and_numbers_print_like_javascript() {
    let mut card = parse("# T\n", "t.md");
    card.assignee = "a \"b\" \\ c\té".into();
    card.labels = vec!["x, y".into(), "z".into()];
    for (order, text) in [
        (3.0, "3"),
        (2.5, "2.5"),
        (1728650000000.0, "1728650000000"),
        (-0.0, "0"),
        (-5.0, "-5"),
    ] {
        card.order = order;
        let out = write(&card, NOW);
        assert!(out.contains(&format!("\norder: {text}\n")), "{out}");
        assert!(out.contains("assignee: \"a \\\"b\\\" \\\\ c\\té\"\n"));
        assert!(out.contains("labels: [\"x, y\", \"z\"]\n"));
        let back = parse(&out, "t.md");
        assert_eq!(
            (back.assignee.as_str(), &back.labels, back.order),
            (card.assignee.as_str(), &card.labels, order)
        );
    }
}

#[test]
fn quick_add() {
    let q = parse_quick_add("Fix login p1 #backend @safi due:friday", SUNDAY);
    assert_eq!(q.title, "Fix login");
    assert_eq!(q.priority, Some(Priority::Critical));
    assert_eq!(q.labels, ["backend"]);
    assert_eq!(q.assignee.as_deref(), Some("safi"));
    assert_eq!(q.due_date.as_deref(), Some("2026-10-16"));

    let due = |s: &str| parse_quick_add(s, SUNDAY).due_date;
    assert_eq!(due("x due:today").as_deref(), Some("2026-10-11"));
    assert_eq!(due("x due:Tomorrow").as_deref(), Some("2026-10-12"));
    assert_eq!(
        due("x due:sunday").as_deref(),
        Some("2026-10-18"),
        "today's weekday means next week"
    );
    assert_eq!(due("x due:2026-12-01").as_deref(), Some("2026-12-01"));
    // An unknown due word stays in the title, as do a bare # and @.
    let q = parse_quick_add("Ship  it due:soon # @ HIGH low", SUNDAY);
    assert_eq!(
        (q.title.as_str(), q.priority, q.due_date),
        ("Ship it due:soon # @", Some(Priority::Low), None)
    );
}

#[test]
fn templates() {
    let (priority, labels, body) = template(Template::Bug, "Crash");
    assert_eq!(
        (priority, labels),
        (Priority::High, vec!["bug".to_string()])
    );
    assert_eq!(
        body,
        "# Crash\n\nSteps:\n- [ ] Repro\n- [ ] Fix\n- [ ] Verify\n"
    );
    assert_eq!(
        template(Template::Feature, "").2,
        "# Untitled\n\n- [ ] Spec\n- [ ] Build\n- [ ] Verify\n"
    );
    assert_eq!(
        template(Template::Chore, "Tidy"),
        (
            Priority::Low,
            vec!["chore".to_string()],
            "# Tidy\n\n- [ ] Do\n- [ ] Verify\n".to_string()
        )
    );
}

#[test]
fn due_labels() {
    let label = |due: &str| format_due(due, SUNDAY);
    assert_eq!(label("2026-10-08"), ("3d overdue".into(), Tone::Red));
    assert_eq!(label("2026-10-10"), ("1d overdue".into(), Tone::Red));
    assert_eq!(label("2026-10-11"), ("Today".into(), Tone::Amber));
    assert_eq!(label("2026-10-12"), ("Tomorrow".into(), Tone::Muted));
    assert_eq!(label("2026-10-14"), ("3d".into(), Tone::Muted));
    assert_eq!(label("2026-11-10"), ("30d".into(), Tone::Muted));
    assert_eq!(label("2026-11-11"), ("2026-11-11".into(), Tone::Muted));
    assert_eq!(label("next week"), ("next week".into(), Tone::Muted));
}

#[test]
fn dates() {
    assert_eq!(Date::parse("1970-01-01").unwrap().days(), 0);
    assert_eq!(SUNDAY.weekday(), 0);
    assert_eq!(
        Date::parse("2024-02-28").unwrap().add_days(2).to_string(),
        "2024-03-01"
    );
    assert_eq!(
        Date::parse("2026-12-31").unwrap().add_days(1).to_string(),
        "2027-01-01"
    );
    for days in [-800_000, -1, 0, 59, 20_000, 800_000] {
        assert_eq!(Date::from_days(days).days(), days);
    }
    assert_eq!(Date::parse("2026-13-01"), None);
    assert_eq!(Date::parse("26-10-11"), None);
    let now = now_iso();
    assert!(
        now.len() == 24 && now.ends_with('Z') && Date::parse(&now[..10]).is_some(),
        "{now}"
    );
}

#[test]
fn slugs_and_file_names() {
    assert_eq!(slugify("  Fix: the Login!! page "), "fix-the-login-page");
    assert_eq!(slugify("日本語"), "task");
    assert_eq!(slugify(&"ab ".repeat(40)).len(), 60);
    assert_eq!(
        new_file_name("Fix login", SUNDAY),
        "fix-login-2026-10-11.md"
    );
}

#[test]
fn comments() {
    assert_eq!(strip_comments("a %%hidden%% b"), "a  b");
    assert_eq!(strip_comments("one\n%% gone %%\ntwo"), "one\ntwo");
    assert_eq!(strip_comments("one\n%% spans\nlines %% two"), "one\n two");
    assert_eq!(
        strip_comments("```\n%% kept %%\n```\nx %%y%%"),
        "```\n%% kept %%\n```\nx "
    );
    assert_eq!(strip_comments("shown %% never closed\nhidden"), "shown ");
    assert_eq!(strip_comments("no comments"), "no comments");
}

#[test]
fn subtask_list_and_toggle() {
    let body = "# T\n- [ ] one\n  * [X] two\ntext\n- [x]three\n-[ ] not one";
    let list = subtasks(body);
    assert_eq!(
        list.iter()
            .map(|s| (s.done, s.text.as_str(), s.line))
            .collect::<Vec<_>>(),
        [(false, "one", 1), (true, "two", 2), (true, "three", 4)]
    );
    assert_eq!(
        toggle_subtask(body, 1),
        body.replace("* [X] two", "* [ ] two")
    );
    assert_eq!(
        toggle_subtask(body, 0),
        body.replace("- [ ] one", "- [x] one")
    );
    assert_eq!(toggle_subtask(body, 9), body);
}

#[test]
fn code_spans_split_on_backtick_pairs() {
    use mdk_format::code_spans;
    assert_eq!(
        code_spans("Use `<video>` here, `PROGRESS.md`"),
        vec![
            ("Use ", false),
            ("<video>", true),
            (" here, ", false),
            ("PROGRESS.md", true)
        ]
    );
    assert_eq!(code_spans("odd ` tick"), vec![("odd ` tick", false)]);
    assert_eq!(code_spans(""), vec![]);
}

#[test]
fn excerpts() {
    assert_eq!(
        excerpt("# Title\n\nFirst %% private %% line.\nSecond."),
        "First line. Second."
    );
    // A real card's body, which used to show its markdown on the card face.
    assert_eq!(
        excerpt(
            "# Export\n\nCovers FR-EXP-01/02/03.\n\n- [ ] Output frame + aspect presets (16:9, 1:1).\n- [x] Run `ffmpeg` **twice**\n* plain bullet\n2. numbered\n## Notes\n> [!note] Heads up\n> See [the spec](https://x.y/spec) and ==this== __too__.\n![shot](pic.png)"
        ),
        "Covers FR-EXP-01/02/03. Output frame + aspect presets (16:9, 1:1). Run ffmpeg twice plain bullet numbered Notes Heads up See the spec and this too. shot"
            .chars()
            .take(140)
            .collect::<String>()
    );
    assert_eq!(
        excerpt("> [!warning]- Folded\n> See [x](y) ==now==."),
        "Folded See x now."
    );
    assert_eq!(excerpt("# Only a title"), "");
    assert_eq!(
        excerpt(&format!("# T\n{}", "é".repeat(200)))
            .chars()
            .count(),
        140
    );
}
