//! The real root view in a headless window, on a temp project folder.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, base::Root};
use mdk::app::{Mdk, window_options};
use mdk::prefs::{Prefs, SavedProject, ThemeChoice};
use mdk_format::Status;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn card(title: &str, status: &str, order: i32) -> String {
    format!(
        "---\nid: \"{title}\"\nstatus: \"{status}\"\npriority: \"high\"\nassignee: \"safi\"\n\
         dueDate: \"2026-01-01\"\nlabels: [\"a\", \"b\", \"c\", \"d\"]\norder: {order}\n---\n\
         # {title}\n\nSome notes.\n\n- [x] one\n- [ ] two\n"
    )
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

struct Fixture {
    project: tempfile::TempDir,
    config: tempfile::TempDir,
    window: WindowHandle<Root>,
    view: Entity<Mdk>,
}

/// `watch` starts the real file watcher. Only the live-update test wants it: the watcher
/// thread wakes the app from outside, which GPUI's test scheduler allows only when parking.
fn open(cx: &mut TestAppContext, watch: bool, prefs: impl FnOnce(&Path) -> Prefs) -> Fixture {
    let project = tempfile::tempdir().unwrap();
    put(
        project.path(),
        "backend/login.md",
        &card("Fix login", "todo", 1),
    );
    put(
        project.path(),
        "backend/logout.md",
        &card("Fix logout", "todo", 2),
    );
    put(
        project.path(),
        "board/drag.md",
        &card("Drag cards", "review", 3),
    );
    put(project.path(), "archive/old.md", &card("Old", "done", 4));
    put(
        project.path(),
        "broken.md",
        "---\nstatus: [todo\n---\n# Broken\n",
    );
    let config = tempfile::tempdir().unwrap();
    let prefs = prefs(project.path());
    let dir = config.path().to_path_buf();

    let (window, view) = cx.update(|cx| {
        gpui_kit::init(cx);
        mdk::theme::init(cx);
        mdk::app::bind_keys(cx);
        let options = window_options(&prefs, cx);
        let (_tx, instance) = futures::channel::mpsc::unbounded();
        std::mem::forget(_tx);
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Mdk::new(prefs, dir, None, instance, watch, window, cx))
        })
        .unwrap()
    });
    let fixture = Fixture {
        project,
        config,
        window: window.downcast::<Root>().unwrap(),
        view,
    };
    settle(cx, &fixture);
    fixture
}

/// Let background scans finish and draw a frame.
fn settle(cx: &mut TestAppContext, f: &Fixture) {
    cx.run_until_parked();
    cx.update_window(f.window.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
}

fn connected(root: &Path) -> Prefs {
    Prefs {
        projects: vec![SavedProject {
            id: "testproj".into(),
            name: "tasks".into(),
            root: root.to_path_buf(),
        }],
        ..Prefs::default()
    }
}

fn column(cx: &mut TestAppContext, f: &Fixture, status: Status) -> Vec<String> {
    cx.update(|cx| {
        f.view
            .read(cx)
            .board
            .column(status)
            .iter()
            .map(|c| c.data.title.clone())
            .collect()
    })
}

#[gpui_kit::test]
fn a_saved_project_loads_into_columns_and_renders(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    assert_eq!(column(cx, &f, Status::Todo), ["Fix login", "Fix logout"]);
    assert_eq!(column(cx, &f, Status::Review), ["Drag cards"]);
    assert_eq!(column(cx, &f, Status::Backlog), ["Broken"]);
    assert!(
        column(cx, &f, Status::Done).is_empty(),
        "archive/ is hidden"
    );
    cx.update(|cx| {
        let view = f.view.read(cx);
        assert_eq!(view.board.card_count(), 5);
        assert_eq!(view.board.open_counts()["testproj"], 4);
        assert_eq!(view.board.workspaces(), ["archive", "backend", "board"]);
    });
}

#[gpui_kit::test]
fn with_no_projects_the_welcome_state_renders(cx: &mut TestAppContext) {
    let f = open(cx, false, |_| Prefs::default());
    cx.update(|cx| assert!(f.view.read(cx).board.projects.is_empty()));
    // Connecting the folder fills the board and selects the project.
    let root = f.project.path().to_path_buf();
    cx.update(|cx| f.view.update(cx, |view, cx| view.connect(&root, cx)));
    settle(cx, &f);
    assert_eq!(column(cx, &f, Status::Todo).len(), 2);
    cx.update(|cx| assert!(f.view.read(cx).board.selected_project.is_some()));
}

#[gpui_kit::test]
fn selection_and_toggles_are_saved_and_restored(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    cx.update_window(f.window.into(), |_, window, cx| {
        f.view.update(cx, |view, cx| {
            view.select_project(Some("testproj".into()), cx);
            view.select_workspace(Some("board".into()), cx);
            view.set_vim_keys(true, cx);
            view.toggle_theme(window, cx);
            view.save_now();
        })
    })
    .unwrap();
    settle(cx, &f);
    assert_eq!(column(cx, &f, Status::Review), ["Drag cards"]);
    assert!(column(cx, &f, Status::Todo).is_empty());

    let (saved, problem) = Prefs::load(f.config.path());
    assert_eq!(problem, None);
    assert_eq!(saved.selected_project, "testproj");
    assert_eq!(saved.selected_workspace, "board");
    assert!(saved.vim_keys);
    assert_eq!(saved.theme, ThemeChoice::Light);
    assert_eq!(saved.projects.len(), 1);

    // A new window started from the saved file comes back the same.
    let again = open(cx, false, |_| {
        let mut prefs = saved.clone();
        prefs.projects[0].root = f.project.path().to_path_buf();
        prefs
    });
    cx.update(|cx| {
        let view = again.view.read(cx);
        assert_eq!(view.board.selected_project.as_deref(), Some("testproj"));
        assert_eq!(view.board.selected_workspace.as_deref(), Some("board"));
    });
}

#[gpui_kit::test]
fn removing_a_project_leaves_its_files(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    cx.update(|cx| {
        f.view
            .update(cx, |view, cx| view.remove_project("testproj", cx))
    });
    settle(cx, &f);
    cx.update(|cx| assert!(f.view.read(cx).board.projects.is_empty()));
    assert!(f.project.path().join("backend/login.md").exists());
}

#[gpui_kit::test]
fn an_edit_by_another_program_moves_the_card(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let f = open(cx, true, connected);
    // The agent moves a card by editing one line of its file.
    std::thread::sleep(Duration::from_millis(400));
    put(
        f.project.path(),
        "backend/login.md",
        &card("Fix login", "done", 1),
    );
    let mut waited = Duration::ZERO;
    while column(cx, &f, Status::Done).is_empty() {
        assert!(waited < Duration::from_secs(5), "the board never updated");
        std::thread::sleep(Duration::from_millis(25));
        waited += Duration::from_millis(25);
        cx.run_until_parked();
    }
    assert_eq!(column(cx, &f, Status::Done), ["Fix login"]);
    assert_eq!(column(cx, &f, Status::Todo), ["Fix logout"]);
    settle(cx, &f);
    // Stop the watcher thread before the test's scheduler goes away.
    cx.update(|cx| f.view.update(cx, |view, _| view.stop_watching()));
    std::thread::sleep(Duration::from_millis(300));
}

#[gpui_kit::test]
fn opening_a_card_shows_the_sheet_and_escape_closes_it(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    let key = cx.update(|cx| f.view.read(cx).board.column(Status::Review)[0].key.clone());
    cx.update(|cx| f.view.update(cx, |view, cx| view.open(key.clone(), cx)));
    settle(cx, &f);
    cx.update(|cx| assert_eq!(f.view.read(cx).open_card.as_ref(), Some(&key)));
    cx.update_window(f.window.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.update(|cx| assert_eq!(f.view.read(cx).open_card, None));
}

#[cfg(unix)]
#[test]
fn a_second_launch_signals_the_first_and_steps_aside() {
    use futures::StreamExt;
    let dir = tempfile::tempdir().unwrap();
    let mut first = mdk::app::single_instance(dir.path()).expect("the first launch runs");
    assert!(
        mdk::app::single_instance(dir.path()).is_none(),
        "the second launch must exit"
    );
    assert_eq!(futures::executor::block_on(first.next()), Some(()));
    // After the first one is gone, its socket file is stale and a new launch takes over.
    drop(first);
    std::fs::remove_file(dir.path().join("mdk.sock")).unwrap();
    assert!(mdk::app::single_instance(dir.path()).is_some());
}

// ---- M3: moving and creating, through the real window ---------------------------------------

fn key_of(cx: &mut TestAppContext, f: &Fixture, status: Status, row: usize) -> mdk::board::CardKey {
    cx.update(|cx| f.view.read(cx).board.column(status)[row].key.clone())
}

fn read(f: &Fixture, rel: &str) -> String {
    fs::read_to_string(f.project.path().join(rel)).unwrap()
}

fn press(cx: &mut TestAppContext, f: &Fixture, keys: &[&str]) {
    for key in keys {
        cx.update_window(f.window.into(), |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    }
}

fn toast(cx: &mut TestAppContext, f: &Fixture) -> Option<(String, bool)> {
    cx.update(|cx| {
        f.view
            .read(cx)
            .toast
            .as_ref()
            .map(|t| (t.text.clone(), t.undo))
    })
}

/// Drag the card at (column, row) and drop it `dy` below the top of the row at `to`.
fn drag(cx: &mut TestAppContext, f: &Fixture, from: (usize, usize), to: (usize, usize), dy: f32) {
    cx.update_window(f.window.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = |(column, row): (usize, usize)| {
            f.view.read(cx).lists[column]
                .bounds_for_item(row)
                .expect("the row is on screen")
        };
        let (a, b) = (bounds(from), bounds(to));
        window.drag(
            a.center(),
            gpui_kit::point(b.center().x, b.top() + gpui_kit::px(dy)),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn dragging_a_card_to_another_column_rewrites_its_file(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    // "Fix logout" (To Do, row 1) dropped on the top edge of Review's only card.
    drag(cx, &f, (1, 1), (3, 0), 4.);
    assert_eq!(column(cx, &f, Status::Review), ["Fix logout", "Drag cards"]);
    assert_eq!(column(cx, &f, Status::Todo), ["Fix login"]);
    let text = read(&f, "backend/logout.md");
    assert!(
        text.contains("status: \"review\"\n") && text.contains("order: 2\n"),
        "{text}"
    );
    assert_eq!(toast(cx, &f), Some(("Moved to Review".into(), true)));
    cx.update(|cx| assert_eq!(f.view.read(cx).slot, None, "the drop slot is cleared"));

    // The toast's Undo and the keyboard shortcut are the same undo.
    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    press(cx, &f, &[undo]);
    assert_eq!(column(cx, &f, Status::Todo), ["Fix login", "Fix logout"]);
    assert_eq!(read(&f, "backend/logout.md"), card("Fix logout", "todo", 2));
    let redo = if cfg!(target_os = "macos") {
        "cmd-shift-z"
    } else {
        "ctrl-shift-z"
    };
    press(cx, &f, &[redo]);
    assert_eq!(column(cx, &f, Status::Review), ["Fix logout", "Drag cards"]);
}

#[gpui_kit::test]
fn dropping_a_card_where_it_already_is_writes_nothing(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    let before = read(&f, "backend/login.md");
    drag(cx, &f, (1, 0), (1, 1), 4.);
    assert_eq!(read(&f, "backend/login.md"), before);
    assert_eq!(toast(cx, &f), None);
}

#[gpui_kit::test]
fn a_card_with_broken_frontmatter_cannot_be_dragged(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    let before = read(&f, "broken.md");
    drag(cx, &f, (0, 0), (3, 0), 4.);
    assert_eq!(column(cx, &f, Status::Backlog), ["Broken"]);
    assert_eq!(read(&f, "broken.md"), before);
    assert_eq!(
        toast(cx, &f),
        Some(("Fix the frontmatter first".into(), false))
    );
}

#[gpui_kit::test]
fn the_keyboard_moves_a_card_and_escape_puts_it_back(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    let original = read(&f, "backend/login.md");
    // Down focuses the first card (Backlog's "Broken"). Right goes to To Do's first card.
    press(cx, &f, &["down", "right"]);
    let login = key_of(cx, &f, Status::Todo, 0);
    cx.update(|cx| assert_eq!(f.view.read(cx).focused.as_ref(), Some(&login)));

    // Space picks it up, Right moves it a column, Down and Up stay inside the column.
    press(cx, &f, &["space", "right"]);
    assert_eq!(column(cx, &f, Status::InProgress), ["Fix login"]);
    press(cx, &f, &["right"]);
    assert_eq!(column(cx, &f, Status::Review), ["Drag cards", "Fix login"]);
    press(cx, &f, &["up"]);
    assert_eq!(column(cx, &f, Status::Review), ["Fix login", "Drag cards"]);
    cx.update(|cx| assert!(f.view.read(cx).grabbed.is_some()));

    // Escape cancels the whole move.
    press(cx, &f, &["escape"]);
    assert_eq!(column(cx, &f, Status::Todo), ["Fix login", "Fix logout"]);
    let restored = read(&f, "backend/login.md");
    assert!(restored.contains("status: \"todo\"\n") && restored.contains("order: 1\n"));
    // Everything after the frontmatter is as it was.
    assert_eq!(
        restored.split("---\n").last(),
        original.split("---\n").last()
    );
    cx.update(|cx| assert!(f.view.read(cx).grabbed.is_none()));

    // Space, Right, Space drops it for good.
    press(cx, &f, &["space", "right", "space"]);
    assert_eq!(column(cx, &f, Status::InProgress), ["Fix login"]);
    press(cx, &f, &["escape"]);
    assert_eq!(
        column(cx, &f, Status::InProgress),
        ["Fix login"],
        "a dropped card stays"
    );
}

#[gpui_kit::test]
fn n_opens_the_dialog_and_quick_add_fills_the_card(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    press(cx, &f, &["n"]);
    cx.update(|cx| assert!(f.view.read(cx).new_card.is_some()));
    // Typing goes to the title field, and N typed there is a letter, not the shortcut.
    cx.update_window(f.window.into(), |_, window, cx| {
        window.input("New nav p1 #backend @dana due:2026-12-01", cx)
    })
    .unwrap();
    press(cx, &f, &["enter"]);
    cx.update(|cx| assert!(f.view.read(cx).new_card.is_none(), "the dialog closes"));

    let name = format!("new-nav-{}.md", mdk::app::today());
    let text = read(&f, &name);
    assert!(
        text.contains("priority: \"critical\"\nassignee: \"dana\"\ndueDate: \"2026-12-01\"\n"),
        "{text}"
    );
    assert!(text.contains("labels: [\"backend\"]\n") && text.ends_with("---\n# New nav\n"));
    assert_eq!(column(cx, &f, Status::Backlog), ["Broken", "New nav"]);

    // Undo removes the file again.
    cx.update(|cx| f.view.update(cx, |view, cx| view.undo(cx)));
    assert!(!f.project.path().join(&name).exists());
}

#[gpui_kit::test]
fn escape_closes_the_dialog_without_creating_anything(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    press(cx, &f, &["n"]);
    cx.update_window(f.window.into(), |_, window, cx| {
        window.input("Half typed", cx)
    })
    .unwrap();
    press(cx, &f, &["escape"]);
    cx.update(|cx| {
        let view = f.view.read(cx);
        assert!(view.new_card.is_none());
        assert_eq!(view.board.card_count(), 5);
    });
    // The board has the keyboard again: N opens a fresh dialog.
    press(cx, &f, &["n"]);
    cx.update(|cx| assert!(f.view.read(cx).new_card.is_some()));
}

#[gpui_kit::test]
fn archive_asks_first_and_moves_the_done_cards(cx: &mut TestAppContext) {
    let f = open(cx, false, connected);
    let login = key_of(cx, &f, Status::Todo, 0);
    cx.update(|cx| {
        f.view.update(cx, |view, cx| {
            view.move_card(&login, Status::Done, None, cx);
            view.ask_archive(cx);
        })
    });
    cx.update(|cx| {
        assert!(matches!(
            f.view.read(cx).confirm,
            Some(mdk::app::Confirm::Archive(1))
        ))
    });
    // Escape answers no.
    press(cx, &f, &["escape"]);
    assert!(f.project.path().join("backend/login.md").exists());

    cx.update_window(f.window.into(), |_, window, cx| {
        f.view.update(cx, |view, cx| {
            view.ask_archive(cx);
            view.answer_confirm(true, window, cx);
        })
    })
    .unwrap();
    assert!(!f.project.path().join("backend/login.md").exists());
    assert!(f.project.path().join("archive/login.md").exists());
    assert!(column(cx, &f, Status::Done).is_empty());
    assert_eq!(
        toast(cx, &f),
        Some(("Archived 1 cards to archive/".into(), true))
    );
}
