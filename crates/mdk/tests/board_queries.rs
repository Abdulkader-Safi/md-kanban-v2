//! Filters and the command palette's item list, on real files.

use mdk::board::{Board, CardKey, Command, DueFilter, Filters, PaletteItem};
use mdk::prefs::SavedView;
use mdk_format::{Date, Priority, Status};
use std::fs;

const TODAY: Date = Date {
    year: 2026,
    month: 10,
    day: 11,
};

fn card(
    title: &str,
    status: &str,
    priority: &str,
    assignee: &str,
    due: &str,
    labels: &str,
    order: i32,
) -> String {
    let due = if due.is_empty() {
        "null".to_string()
    } else {
        format!("\"{due}\"")
    };
    format!(
        "---\nid: \"{}\"\nstatus: \"{status}\"\npriority: \"{priority}\"\nassignee: \"{assignee}\"\ndueDate: {due}\nlabels: [{labels}]\norder: {order}\n---\n# {title}\n\nBody of {title}.\n",
        title.to_lowercase().replace(' ', "-")
    )
}

fn board() -> (tempfile::TempDir, Board, String) {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        (
            "login.md",
            card(
                "Fix login",
                "todo",
                "high",
                "safi",
                "2026-10-08",
                "\"auth\", \"backend\"",
                1,
            ),
        ),
        (
            "logout.md",
            card(
                "Fix logout",
                "review",
                "high",
                "Dana",
                "2026-10-11",
                "\"auth\"",
                2,
            ),
        ),
        (
            "pricing.md",
            card(
                "Pricing page",
                "backlog",
                "low",
                "safi",
                "2026-10-15",
                "\"ui\"",
                3,
            ),
        ),
        (
            "sync.md",
            card("Research sync", "todo", "critical", "", "", "", 4),
        ),
        (
            "later.md",
            card(
                "Plan next year",
                "backlog",
                "medium",
                "safi",
                "2026-10-18",
                "\"ui\"",
                5,
            ),
        ),
        (
            "odd.md",
            card("Odd due", "done", "medium", "", "next week", "", 6),
        ),
        (
            "archive/old.md",
            card(
                "Old login",
                "done",
                "high",
                "safi",
                "2026-01-01",
                "\"auth\"",
                7,
            ),
        ),
    ];
    for (rel, text) in files {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let mut board = Board::default();
    let id = board.connect_project(dir.path()).unwrap();
    board.scan_now(&id);
    board.today = Some(TODAY);
    (dir, board, id)
}

fn titles(board: &Board) -> Vec<&str> {
    board
        .visible_cards()
        .iter()
        .map(|c| c.data.title.as_str())
        .collect()
}

fn with(board: &mut Board, filters: Filters) -> Vec<String> {
    board.filters = filters;
    titles(board).into_iter().map(str::to_string).collect()
}

#[test]
fn each_filter_alone() {
    let (_dir, mut b, _) = board();
    assert_eq!(titles(&b).len(), 6, "archive/ is hidden");
    assert!(!b.filters.is_active());

    let f = |change: fn(&mut Filters)| {
        let mut filters = Filters::default();
        change(&mut filters);
        filters
    };
    assert_eq!(
        with(&mut b, f(|f| f.query = "  LOGIN ".into())),
        ["Fix login"]
    );
    assert_eq!(
        with(&mut b, f(|f| f.query = "auth".into())),
        ["Fix login", "Fix logout"],
        "labels are searched"
    );
    assert_eq!(
        with(&mut b, f(|f| f.query = "dana".into())),
        ["Fix logout"],
        "so is the assignee"
    );
    assert_eq!(
        with(&mut b, f(|f| f.query = "body of pricing".into())),
        ["Pricing page"],
        "and the body"
    );
    assert_eq!(
        with(&mut b, f(|f| f.priorities = vec![Priority::High])),
        ["Fix login", "Fix logout"]
    );
    assert_eq!(
        with(
            &mut b,
            f(|f| f.priorities = vec![Priority::Critical, Priority::Low])
        ),
        ["Pricing page", "Research sync"]
    );
    assert_eq!(
        with(&mut b, f(|f| f.assignee = "DANA".into())),
        ["Fix logout"],
        "exact, ignoring case"
    );
    assert!(
        with(&mut b, f(|f| f.assignee = "dan".into())).is_empty(),
        "not a prefix match"
    );
    assert_eq!(
        with(&mut b, f(|f| f.label = Some("ui".into()))),
        ["Pricing page", "Plan next year"]
    );
    assert_eq!(
        with(&mut b, f(|f| f.due = DueFilter::Overdue)),
        ["Fix login"]
    );
    assert_eq!(
        with(&mut b, f(|f| f.due = DueFilter::Today)),
        ["Fix logout"]
    );
    assert_eq!(
        with(&mut b, f(|f| f.due = DueFilter::Week)),
        ["Fix logout", "Pricing page"],
        "today and the six days after"
    );
    assert_eq!(
        with(&mut b, f(|f| f.due = DueFilter::None)),
        ["Research sync"]
    );
    assert!(b.filters.is_active());
}

#[test]
fn filters_combine_and_reset() {
    let (_dir, mut b, _) = board();
    let overdue_high = Filters {
        priorities: vec![Priority::High],
        due: DueFilter::Overdue,
        ..Default::default()
    };
    assert_eq!(with(&mut b, overdue_high), ["Fix login"]);
    // The same filter applies in every column.
    assert_eq!(b.column(Status::Todo).len(), 1);
    assert!(b.column(Status::Review).is_empty());

    let none = Filters {
        query: "login".into(),
        label: Some("ui".into()),
        ..Default::default()
    };
    assert!(with(&mut b, none).is_empty());
    assert_eq!(with(&mut b, Filters::default()).len(), 6);
}

#[test]
fn the_label_list_follows_the_selection_not_the_filters() {
    let (_dir, mut b, _) = board();
    assert_eq!(b.labels_in_view(), ["auth", "backend", "ui"]);
    b.filters.query = "pricing".into();
    assert_eq!(b.labels_in_view(), ["auth", "backend", "ui"]);
    b.select_workspace(Some("archive".into()));
    assert_eq!(b.labels_in_view(), ["auth"]);
}

#[test]
fn a_saved_view_round_trips_through_the_config_file() {
    let filters = Filters {
        query: "login".into(),
        priorities: vec![Priority::High, Priority::Critical],
        assignee: "safi".into(),
        label: Some("auth".into()),
        due: DueFilter::Overdue,
    };
    let view = SavedView::new("  my overdue ", &filters);
    assert_eq!(view.name, "my overdue");
    let json = serde_json::to_string(&view).unwrap();
    let back: SavedView = serde_json::from_str(&json).unwrap();
    assert_eq!(back.filters(), filters);
    // A view written by hand with words mdk does not know falls back safely.
    let odd: SavedView =
        serde_json::from_str(r#"{"name":"x","priorities":["urgent","low"],"due":"someday"}"#)
            .unwrap();
    assert_eq!(
        odd.filters(),
        Filters {
            priorities: vec![Priority::Low],
            ..Default::default()
        }
    );
}

#[test]
fn palette_items() {
    let (_dir, b, id) = board();
    let key = |rel: &str| CardKey {
        project: id.clone(),
        rel_path: rel.into(),
    };

    // No query: the first 8 cards, then every app command. Archived cards are left out.
    let all = b.palette_items("", None);
    assert_eq!(
        all.iter()
            .filter(|i| matches!(i, PaletteItem::Card(_)))
            .count(),
        6
    );
    assert_eq!(
        all.iter()
            .filter(|i| matches!(i, PaletteItem::Command(_)))
            .count(),
        Command::ALL.len()
    );
    assert!(
        !all.iter()
            .any(|i| matches!(i, PaletteItem::Create(_) | PaletteItem::Move(_)))
    );

    let login = b.palette_items("login", None);
    assert_eq!(login[0], PaletteItem::Card(key("login.md")));
    assert_eq!(login[1], PaletteItem::Create("login".into()));
    assert_eq!(login.len(), 2);

    // With a card open, the other four columns are offered.
    let open = key("login.md");
    let moves: Vec<_> = b
        .palette_items("move open", Some(&open))
        .into_iter()
        .filter_map(|i| match i {
            PaletteItem::Move(status) => Some(status),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves,
        [
            Status::Backlog,
            Status::InProgress,
            Status::Review,
            Status::Done
        ]
    );
    // Status is not searched, so "done" finds no card: only the move and the command.
    assert_eq!(
        b.palette_items("done", Some(&open)),
        [
            PaletteItem::Create("done".into()),
            PaletteItem::Move(Status::Done),
            PaletteItem::Command(Command::ArchiveDone),
        ]
    );
    assert_eq!(
        b.palette_items("vim", None).last(),
        Some(&PaletteItem::Command(Command::ToggleVim))
    );
}

#[test]
fn the_palette_caps_cards_at_eight() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..20 {
        fs::write(
            dir.path().join(format!("c{i:02}.md")),
            card(&format!("Card {i:02}"), "todo", "low", "", "", "", i),
        )
        .unwrap();
    }
    let mut b = Board::default();
    let id = b.connect_project(dir.path()).unwrap();
    b.scan_now(&id);
    let items = b.palette_items("card", None);
    let cards: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, PaletteItem::Card(_)))
        .collect();
    assert_eq!(cards.len(), 8);
    assert_eq!(
        items[0],
        PaletteItem::Card(CardKey {
            project: id,
            rel_path: "c00.md".into()
        })
    );
}
