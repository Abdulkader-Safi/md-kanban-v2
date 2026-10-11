//! The board's write operations against real files: what a move writes, what survives it,
//! and that undo puts back the exact bytes.

use mdk::board::{Board, CardKey, NewCard};
use mdk_format::{CardPatch, Date, Priority, Status};
use std::fs;
use std::path::Path;

const TODAY: Date = Date {
    year: 2026,
    month: 10,
    day: 11,
};

fn card(status: &str, order: f64) -> String {
    format!(
        "---\nid: \"c\"\nstatus: \"{status}\"\npriority: \"low\"\nassignee: \"\"\ndueDate: null\n\
         created: \"2026-10-01T08:00:00.000Z\"\nmodified: \"2026-10-01T08:00:00.000Z\"\n\
         labels: []\norder: {order}\nsprint: 4\nagent: \"claude\"\n---\n# Card\n\nNotes.\n"
    )
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

struct Fx {
    dir: tempfile::TempDir,
    board: Board,
    id: String,
}

impl Fx {
    fn new(files: &[(&str, String)]) -> Fx {
        let dir = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            put(dir.path(), rel, text);
        }
        let mut board = Board::default();
        let id = board.connect_project(dir.path()).unwrap();
        board.scan_now(&id);
        Fx { dir, board, id }
    }

    fn key(&self, rel: &str) -> CardKey {
        CardKey {
            project: self.id.clone(),
            rel_path: rel.into(),
        }
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.dir.path().join(rel)).unwrap()
    }

    fn exists(&self, rel: &str) -> bool {
        self.dir.path().join(rel).exists()
    }

    fn order(&self, rel: &str) -> f64 {
        self.board.card(&self.key(rel)).unwrap().data.order
    }

    fn column(&self, status: Status) -> Vec<String> {
        self.board
            .column(status)
            .iter()
            .map(|c| c.key.rel_path.clone())
            .collect()
    }

    /// Every file under the project, relative, sorted.
    fn files(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut dirs = vec![self.dir.path().to_path_buf()];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    let rel = path.strip_prefix(self.dir.path()).unwrap();
                    // Forward slashes on every OS, like the store's own paths.
                    out.push(rel.to_str().unwrap().replace('\\', "/"));
                }
            }
        }
        out.sort();
        out
    }
}

/// The lines that differ between two versions of a file, by their key.
fn changed_keys(before: &str, after: &str) -> Vec<String> {
    let mut keys: Vec<String> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.split(':').next().unwrap().to_string())
        .collect();
    assert_eq!(
        before.lines().count(),
        after.lines().count(),
        "lines were added or removed"
    );
    keys.sort();
    keys
}

#[test]
fn a_move_lands_between_its_neighbours_and_writes_three_fields() {
    let mut fx = Fx::new(&[
        ("a.md", card("backlog", 1.0)),
        ("r1.md", card("review", 10.0)),
        ("r2.md", card("review", 20.0)),
        ("r3.md", card("review", 30.0)),
    ]);
    let before = fx.read("a.md");
    assert!(
        fx.board
            .move_card(&fx.key("a.md"), Status::Review, Some(2))
            .unwrap()
    );

    assert_eq!(
        fx.column(Status::Review),
        ["r1.md", "r2.md", "a.md", "r3.md"]
    );
    assert_eq!(fx.order("a.md"), 25.0);
    let after = fx.read("a.md");
    assert_eq!(
        changed_keys(&before, &after),
        ["modified", "order", "status"]
    );
    assert!(
        after.contains("\nsprint: 4\nagent: \"claude\"\n---\n# Card\n\nNotes.\n"),
        "{after}"
    );
    // The neighbours were not touched.
    assert_eq!(fx.read("r2.md"), card("review", 20.0));
}

#[test]
fn moves_to_the_top_the_end_and_an_empty_column() {
    let mut fx = Fx::new(&[
        ("a.md", card("todo", 5.0)),
        ("b.md", card("todo", 7.0)),
        ("c.md", card("backlog", 3.0)),
    ]);
    fx.board
        .move_card(&fx.key("c.md"), Status::Todo, Some(0))
        .unwrap();
    assert_eq!(
        (fx.column(Status::Todo), fx.order("c.md")),
        (vec!["c.md".into(), "a.md".into(), "b.md".into()], 4.0)
    );

    fx.board
        .move_card(&fx.key("c.md"), Status::Todo, None)
        .unwrap();
    assert_eq!(
        (
            fx.column(Status::Todo).last().unwrap().as_str(),
            fx.order("c.md")
        ),
        ("c.md", 8.0)
    );
    // Already last: nothing is written.
    let text = fx.read("c.md");
    assert!(
        !fx.board
            .move_card(&fx.key("c.md"), Status::Todo, None)
            .unwrap()
    );
    assert!(
        !fx.board
            .move_card(&fx.key("c.md"), Status::Todo, Some(9))
            .unwrap()
    );
    assert_eq!(fx.read("c.md"), text);

    fx.board
        .move_card(&fx.key("a.md"), Status::Done, None)
        .unwrap();
    assert_eq!(fx.order("a.md"), 0.0);
}

#[test]
fn colliding_orders_renumber_the_column() {
    let mut fx = Fx::new(&[
        ("x.md", card("backlog", 1.0)),
        ("t1.md", card("todo", 5.0)),
        ("t2.md", card("todo", 5.0)),
        ("t3.md", card("todo", 6.0)),
    ]);
    fx.board
        .move_card(&fx.key("x.md"), Status::Todo, Some(1))
        .unwrap();
    assert_eq!(fx.column(Status::Todo), ["t1.md", "x.md", "t2.md", "t3.md"]);
    assert_eq!(
        [
            fx.order("t1.md"),
            fx.order("x.md"),
            fx.order("t2.md"),
            fx.order("t3.md")
        ],
        [0.0, 5.0, 10.0, 20.0]
    );
    // One undo puts the whole column back, byte for byte.
    fx.board.undo().unwrap();
    for (rel, order) in [("t1.md", 5.0), ("t2.md", 5.0), ("t3.md", 6.0)] {
        assert_eq!(fx.read(rel), card("todo", order));
    }
    assert_eq!(fx.read("x.md"), card("backlog", 1.0));
}

#[test]
fn a_move_keeps_what_an_agent_wrote_since_the_last_read() {
    let mut fx = Fx::new(&[("a.md", card("todo", 1.0))]);
    // The agent rewrites the body and adds a key. The board has not seen it yet.
    let by_agent = card("todo", 1.0)
        .replace("Notes.", "Done by the agent.\n\n- [x] shipped")
        .replace("sprint: 4", "sprint: 5\nreviewed: true");
    put(fx.dir.path(), "a.md", &by_agent);

    fx.board
        .move_card(&fx.key("a.md"), Status::Done, None)
        .unwrap();
    let after = fx.read("a.md");
    assert!(after.contains("status: \"done\""));
    assert!(after.contains("sprint: 5\nreviewed: true\nagent: \"claude\""));
    assert!(after.ends_with("# Card\n\nDone by the agent.\n\n- [x] shipped\n"));
    assert_eq!(fx.board.card(&fx.key("a.md")).unwrap().subtasks, (1, 1));
}

#[test]
fn a_card_with_broken_frontmatter_cannot_be_moved_or_rewritten() {
    let broken = "---\nstatus: [todo\nsprint: 4\n---\n# Broken\n";
    let mut fx = Fx::new(&[("broken.md", broken.into())]);
    let key = fx.key("broken.md");
    assert_eq!(
        fx.board.move_card(&key, Status::Done, None).unwrap_err(),
        "Fix the frontmatter first"
    );
    assert!(
        fx.board
            .update_card(
                &key,
                &CardPatch {
                    priority: Some(Priority::High),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(fx.board.toggle_subtask(&key, 0).is_err());
    assert_eq!(fx.read("broken.md"), broken);
    assert!(!fx.board.can_undo());
}

#[test]
fn undo_and_redo_of_a_move_restore_the_exact_bytes() {
    let mut fx = Fx::new(&[("a.md", card("todo", 1.0))]);
    let before = fx.read("a.md");
    fx.board
        .move_card(&fx.key("a.md"), Status::Done, None)
        .unwrap();
    let moved = fx.read("a.md");

    assert_eq!(fx.board.undo().unwrap().as_deref(), Some("Move to Done"));
    assert_eq!(fx.read("a.md"), before);
    assert_eq!(fx.column(Status::Todo), ["a.md"]);
    assert_eq!(fx.board.redo().unwrap().as_deref(), Some("Move to Done"));
    assert_eq!(fx.read("a.md"), moved);
    assert_eq!(fx.column(Status::Done), ["a.md"]);
    assert_eq!(fx.board.redo().unwrap(), None);
}

#[test]
fn undo_is_refused_when_the_file_changed_since() {
    let mut fx = Fx::new(&[("a.md", card("todo", 1.0))]);
    fx.board
        .move_card(&fx.key("a.md"), Status::Done, None)
        .unwrap();
    let by_agent = fx
        .read("a.md")
        .replace("Notes.", "The agent got here first.");
    put(fx.dir.path(), "a.md", &by_agent);

    assert_eq!(
        fx.board.undo().unwrap_err(),
        "Changed on disk since, can't undo"
    );
    assert_eq!(fx.read("a.md"), by_agent, "the agent's edit must survive");
    assert!(
        fx.board.can_undo(),
        "the entry stays, in case the file comes back"
    );
}

#[test]
fn creating_cards_names_files_and_fills_the_format() {
    let mut fx = Fx::new(&[("backend/x.md", card("todo", 1.0))]);
    fx.board.select_workspace(Some("backend".into()));
    let input = NewCard {
        title: "Fix login".into(),
        status: Some(Status::Todo),
        priority: Some(Priority::Critical),
        assignee: "safi".into(),
        due_date: Some("2026-10-16".into()),
        labels: vec!["backend".into()],
        ..Default::default()
    };
    let key = fx.board.create_card(input.clone(), None, TODAY).unwrap();
    assert_eq!(key.rel_path, "backend/fix-login-2026-10-11.md");
    let text = fx.read(&key.rel_path);
    assert!(text.starts_with(
        "---\nid: \"fix-login-2026-10-11\"\nstatus: \"todo\"\npriority: \"critical\"\n\
         assignee: \"safi\"\ndueDate: \"2026-10-16\"\ncreated: \""
    ));
    // New cards sort last: the order is the time in milliseconds.
    assert!(text.contains("labels: [\"backend\"]\norder: 1"), "{text}");
    assert!(fx.board.card(&key).unwrap().data.order > 1.7e12);
    assert!(text.ends_with("\n---\n# Fix login\n"));
    assert_eq!(
        fx.column(Status::Todo),
        ["backend/x.md", "backend/fix-login-2026-10-11.md"],
        "new cards go last"
    );

    // The same title again takes -2. An explicit work folder wins. No folder means the root.
    assert_eq!(
        fx.board
            .create_card(input.clone(), None, TODAY)
            .unwrap()
            .rel_path,
        "backend/fix-login-2026-10-11-2.md"
    );
    assert_eq!(
        fx.board
            .create_card(input.clone(), Some("ui"), TODAY)
            .unwrap()
            .rel_path,
        "ui/fix-login-2026-10-11.md"
    );
    assert_eq!(
        fx.board
            .create_card(input, Some(""), TODAY)
            .unwrap()
            .rel_path,
        "fix-login-2026-10-11.md"
    );
    let second = fx
        .board
        .card(&fx.key("backend/fix-login-2026-10-11-2.md"))
        .unwrap();
    assert_eq!(second.data.id, "fix-login-2026-10-11-2");
    assert!(
        fx.board
            .create_card(
                NewCard {
                    title: "  ".into(),
                    ..Default::default()
                },
                None,
                TODAY
            )
            .is_err()
    );

    // A body with its own heading is kept as written.
    let with_body = NewCard {
        title: "Crash".into(),
        body: "# Crash\n\nSteps:\n- [ ] Repro\n".into(),
        ..Default::default()
    };
    let key = fx.board.create_card(with_body, Some(""), TODAY).unwrap();
    assert!(
        fx.read(&key.rel_path)
            .ends_with("---\n# Crash\n\nSteps:\n- [ ] Repro\n")
    );
    assert_eq!(fx.board.card(&key).unwrap().data.status, Status::Backlog);
}

#[test]
fn undo_of_a_create_removes_the_file_and_redo_brings_it_back() {
    let mut fx = Fx::new(&[("x.md", card("todo", 1.0))]);
    let key = fx
        .board
        .create_card(
            NewCard {
                title: "New".into(),
                ..Default::default()
            },
            None,
            TODAY,
        )
        .unwrap();
    let text = fx.read(&key.rel_path);
    fx.board.undo().unwrap();
    assert!(!fx.exists(&key.rel_path) && fx.board.card(&key).is_none());
    fx.board.redo().unwrap();
    assert_eq!(fx.read(&key.rel_path), text);
}

#[test]
fn undo_of_a_delete_restores_the_file_byte_for_byte() {
    // Odd on purpose: CRLF, a BOM, single quotes, a comment. None of it may be normalised.
    let odd = "\u{feff}---\r\n# by hand\r\nid: 'odd'\r\nstatus: todo\r\nextra:   [1,2]\r\n---\r\n\r\n\r\n# Odd card\r\n\r\ntrailing spaces   \r\n\r\n";
    let mut fx = Fx::new(&[("odd.md", odd.into())]);
    let key = fx.key("odd.md");
    fx.board.delete_card(&key).unwrap();
    assert!(!fx.exists("odd.md") && fx.board.card(&key).is_none());

    assert_eq!(fx.board.undo().unwrap().as_deref(), Some("Delete Odd card"));
    assert_eq!(
        fs::read(fx.dir.path().join("odd.md")).unwrap(),
        odd.as_bytes()
    );
    assert_eq!(fx.board.card(&key).unwrap().data.title, "Odd card");
    fx.board.redo().unwrap();
    assert!(!fx.exists("odd.md"));
}

#[test]
fn archive_moves_done_cards_without_changing_them_and_undo_moves_them_back() {
    let mut files: Vec<(String, String)> = (0..12)
        .map(|i| (format!("work/d{i:02}.md"), card("done", f64::from(i))))
        .collect();
    files.push(("work/open.md".into(), card("todo", 1.0)));
    files.push(("archive/d00.md".into(), card("done", 99.0)));
    let files: Vec<(&str, String)> = files.iter().map(|(p, t)| (p.as_str(), t.clone())).collect();
    let mut fx = Fx::new(&files);
    assert_eq!(fx.board.archivable().len(), 12);

    assert_eq!(fx.board.archive_done().unwrap(), 12);
    assert!(fx.column(Status::Done).is_empty());
    assert_eq!(fx.column(Status::Todo), ["work/open.md"]);
    // The name that was taken gets a suffix. Content is untouched.
    assert_eq!(fx.read("archive/d00-2.md"), card("done", 0.0));
    assert_eq!(fx.read("archive/d00.md"), card("done", 99.0));
    assert_eq!(fx.read("archive/d11.md"), card("done", 11.0));
    assert_eq!(
        fx.files().iter().filter(|f| f.starts_with("work/")).count(),
        1
    );
    assert_eq!(fx.board.archive_done().unwrap(), 0);

    assert_eq!(
        fx.board.undo().unwrap().as_deref(),
        Some("Archive 12 cards")
    );
    assert_eq!(fx.column(Status::Done).len(), 12);
    assert_eq!(fx.read("work/d00.md"), card("done", 0.0));
    assert!(!fx.exists("archive/d00-2.md") && fx.exists("archive/d00.md"));
    fx.board.redo().unwrap();
    assert!(fx.column(Status::Done).is_empty());
}

#[test]
fn ticking_a_subtask_changes_one_character_of_the_body() {
    let text = card("todo", 1.0).replace("Notes.", "- [ ] one\n- [ ] two\n- [ ] three");
    let mut fx = Fx::new(&[("a.md", text.clone())]);
    fx.board.toggle_subtask(&fx.key("a.md"), 1).unwrap();
    let after = fx.read("a.md");
    assert!(after.ends_with("- [ ] one\n- [x] two\n- [ ] three\n"));
    assert_eq!(changed_keys(&text, &after), ["- [ ] two", "modified"]);
    assert_eq!(fx.board.card(&fx.key("a.md")).unwrap().subtasks, (1, 3));
}

#[test]
fn a_session_leaves_only_card_files_in_the_folder() {
    let mut fx = Fx::new(&[("a.md", card("todo", 1.0)), ("b.md", card("done", 2.0))]);
    let new = fx
        .board
        .create_card(
            NewCard {
                title: "New".into(),
                ..Default::default()
            },
            Some("ui"),
            TODAY,
        )
        .unwrap();
    fx.board
        .move_card(&fx.key("a.md"), Status::Review, None)
        .unwrap();
    fx.board.delete_card(&new).unwrap();
    fx.board.archive_done().unwrap();
    fx.board.undo().unwrap();
    fx.board.redo().unwrap();
    assert_eq!(fx.files(), ["a.md", "archive/b.md"]);
}

#[test]
fn a_broken_file_is_fixed_by_writing_its_raw_text() {
    let broken = "---\nstatus: [todo\n---\n# Broken\n";
    let mut fx = Fx::new(&[("broken.md", broken.into())]);
    let key = fx.key("broken.md");
    let fixed = "---\nstatus: \"todo\"\n---\n# Broken\n";
    fx.board.write_raw(&key, fixed).unwrap();
    assert_eq!(fx.read("broken.md"), fixed);
    let card = fx.board.card(&key).unwrap();
    assert_eq!((card.error(), card.data.status), (None, Status::Todo));
    // Now it moves like any card, and both steps undo.
    fx.board.move_card(&key, Status::Done, None).unwrap();
    fx.board.undo().unwrap();
    fx.board.undo().unwrap();
    assert_eq!(fx.read("broken.md"), broken);

    // Writing the same text again is not an edit. A deleted file can be written back.
    fx.board.write_raw(&key, broken).unwrap();
    assert!(!fx.board.can_undo());
    fs::remove_file(fx.dir.path().join("broken.md")).unwrap();
    fx.board.write_raw(&key, fixed).unwrap();
    assert_eq!(fx.read("broken.md"), fixed);
}
