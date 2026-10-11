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

fn open(cx: &mut TestAppContext, prefs: impl FnOnce(&Path) -> Prefs) -> Fixture {
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
            cx.new(|cx| Mdk::new(prefs, dir, None, instance, window, cx))
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
    let f = open(cx, connected);
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
    let f = open(cx, |_| Prefs::default());
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
    let f = open(cx, connected);
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
    let again = open(cx, |_| {
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
    let f = open(cx, connected);
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
    let f = open(cx, connected);
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
}

#[gpui_kit::test]
fn opening_a_card_shows_the_sheet_and_escape_closes_it(cx: &mut TestAppContext) {
    let f = open(cx, connected);
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
