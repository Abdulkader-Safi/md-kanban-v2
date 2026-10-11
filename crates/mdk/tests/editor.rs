//! The card editor in a headless window: what it writes, when, and what it leaves alone.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, base::Root};
use mdk::app::{Confirm, Mdk, window_options};
use mdk::board::CardKey;
use mdk::prefs::{Prefs, SavedProject};
use mdk::views::editor::Tool;
use mdk_format::Status;
use mdk_store::StoreEvent;
use std::fs;
use std::path::Path;
use std::time::Duration;

const CARD: &str = "---\nid: \"login\"\nstatus: \"todo\"\npriority: \"high\"\nassignee: \"safi\"\ndueDate: \"2026-12-01\"\ncreated: \"2026-10-01T08:00:00.000Z\"\nmodified: \"2026-10-01T08:00:00.000Z\"\nlabels: [\"auth\"]\norder: 1\nsprint: 4\nagent: \"claude\"\n---\n# Fix login\n\nNotes here.\n";
const OTHER: &str =
    "---\nid: \"other\"\nstatus: \"review\"\norder: 2\n---\n# Other card\n\nOther notes.\n";
const BROKEN: &str = "---\nstatus: [todo\n---\n# Broken\n";

struct Fx {
    project: tempfile::TempDir,
    _config: tempfile::TempDir,
    window: WindowHandle<Root>,
    view: Entity<Mdk>,
}

fn open(cx: &mut TestAppContext) -> Fx {
    let project = tempfile::tempdir().unwrap();
    for (name, text) in [
        ("login.md", CARD),
        ("other.md", OTHER),
        ("broken.md", BROKEN),
    ] {
        fs::write(project.path().join(name), text).unwrap();
    }
    let config = tempfile::tempdir().unwrap();
    let prefs = Prefs {
        projects: vec![SavedProject {
            id: "proj".into(),
            name: "tasks".into(),
            root: project.path().to_path_buf(),
        }],
        ..Prefs::default()
    };
    let dir = config.path().to_path_buf();
    let (window, view) = cx.update(|cx| {
        gpui_kit::init(cx);
        mdk::theme::init(cx);
        mdk::app::bind_keys(cx);
        let options = window_options(&prefs, cx);
        let (tx, instance) = futures::channel::mpsc::unbounded();
        std::mem::forget(tx);
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Mdk::new(prefs, dir, None, instance, false, window, cx))
        })
        .unwrap()
    });
    let fx = Fx {
        project,
        _config: config,
        window: window.downcast::<Root>().unwrap(),
        view,
    };
    fx.settle(cx);
    fx
}

fn key(rel: &str) -> CardKey {
    CardKey {
        project: "proj".into(),
        rel_path: rel.into(),
    }
}

impl Fx {
    /// Run pending work and draw a frame, which is when the editor meets disk changes.
    fn settle(&self, cx: &mut TestAppContext) {
        cx.run_until_parked();
        cx.update_window(self.window.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }

    /// Run something on the root view with the window.
    fn with<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Mdk, &mut gpui_kit::Window, &mut gpui_kit::Context<Mdk>) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| {
            self.view.update(cx, |view, cx| f(view, window, cx))
        })
        .unwrap()
    }

    fn open_card(&self, cx: &mut TestAppContext, rel: &str) {
        self.with(cx, |view, window, cx| view.open(key(rel), window, cx));
        self.settle(cx);
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.project.path().join(rel)).unwrap()
    }

    fn path(&self, rel: &str) -> std::path::PathBuf {
        self.project.path().join(rel)
    }

    /// Put the keyboard in the body editor with the cursor at the end, and type.
    fn type_in_body(&self, cx: &mut TestAppContext, text: &str) {
        self.with(cx, |view, window, cx| {
            let body = view.editor.as_ref().unwrap().body.clone();
            body.update(cx, |state, cx| {
                state.focus(window, cx);
                let end = state.value().len();
                state.set_selected_range(end..end, cx);
            });
        });
        cx.update_window(self.window.into(), |_, window, cx| window.input(text, cx))
            .unwrap();
    }

    fn body(&self, cx: &mut TestAppContext) -> String {
        cx.update(|cx| {
            self.view
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .body
                .read(cx)
                .value()
                .to_string()
        })
    }

    fn title(&self, cx: &mut TestAppContext) -> String {
        cx.update(|cx| {
            self.view
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .title
                .read(cx)
                .value()
                .to_string()
        })
    }

    /// Replace the whole body the way a user would: select all, then type.
    fn set_body(&self, cx: &mut TestAppContext, text: &str) {
        self.with(cx, |view, window, cx| {
            let body = view.editor.as_ref().unwrap().body.clone();
            body.update(cx, |state, cx| {
                state.focus(window, cx);
                let end = state.value().len();
                state.set_selected_range(0..end, cx);
                state.replace(text.to_string(), window, cx);
            });
        });
        cx.run_until_parked();
    }

    /// Replace an input's text the way a user would: select all, then type.
    fn retype(
        &self,
        cx: &mut TestAppContext,
        pick: impl FnOnce(&mdk::views::CardEditor) -> Entity<gpui_kit::component::input::InputState>,
        text: &str,
    ) {
        self.with(cx, |view, window, cx| {
            let input = pick(view.editor.as_ref().unwrap());
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                let end = state.value().len();
                state.set_selected_range(0..end, cx);
                state.replace(text.to_string(), window, cx);
            });
        });
        cx.run_until_parked();
    }

    fn set_title(&self, cx: &mut TestAppContext, text: &str) {
        self.retype(cx, |editor| editor.title.clone(), text);
    }

    /// Let the 500 ms autosave fire.
    fn wait_for_autosave(&self, cx: &mut TestAppContext) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(600));
        self.settle(cx);
    }

    /// Another program rewrote a card file, and the watcher has noticed.
    fn external_write(&self, cx: &mut TestAppContext, rel: &str, text: &str) {
        fs::write(self.path(rel), text).unwrap();
        self.watcher_saw(
            cx,
            StoreEvent::Changed {
                rel_path: rel.into(),
            },
        );
    }

    fn watcher_saw(&self, cx: &mut TestAppContext, event: StoreEvent) {
        cx.update(|cx| {
            self.view.update(cx, |view, cx| {
                view.board.apply_events("proj", vec![event]);
                view.changed(cx);
            })
        });
        self.settle(cx);
    }

    fn flags(&self, cx: &mut TestAppContext) -> (bool, bool, bool) {
        cx.update(|cx| {
            let editor = self.view.read(cx).editor.as_ref().unwrap();
            (editor.conflict, editor.deleted, editor.raw_mode)
        })
    }
}

fn changed_keys(before: &str, after: &str) -> Vec<String> {
    assert_eq!(before.lines().count(), after.lines().count(), "{after}");
    before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.split(':').next().unwrap().to_string())
        .collect()
}

#[gpui_kit::test]
fn opening_and_closing_a_card_writes_nothing(cx: &mut TestAppContext) {
    let fx = open(cx);
    for rel in ["login.md", "other.md", "broken.md"] {
        fx.open_card(cx, rel);
        fx.wait_for_autosave(cx);
        fx.with(cx, |view, window, cx| view.close_editor(window, cx));
    }
    assert_eq!(fx.read("login.md"), CARD);
    assert_eq!(fx.read("other.md"), OTHER);
    assert_eq!(fx.read("broken.md"), BROKEN);
    cx.update(|cx| assert!(!fx.view.read(cx).board.can_undo()));
}

#[gpui_kit::test]
fn typing_then_closing_at_once_still_saves(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, "\nTyped just before closing.");
    // No time passes: the 500 ms timer has not fired.
    fx.with(cx, |view, window, cx| view.close_editor(window, cx));
    let after = fx.read("login.md");
    assert!(
        after.ends_with("# Fix login\n\nNotes here.\nTyped just before closing.\n"),
        "{after}"
    );
    // Only the body and `modified` changed. The agent's own keys are still there.
    assert!(after.contains(
        "status: \"todo\"\npriority: \"high\"\nassignee: \"safi\"\ndueDate: \"2026-12-01\"\n"
    ));
    assert!(after.contains("labels: [\"auth\"]\norder: 1\nsprint: 4\nagent: \"claude\"\n---\n"));
}

#[gpui_kit::test]
fn edits_autosave_half_a_second_after_the_last_change(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " More.");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(fx.read("login.md"), CARD, "too early to save");
    fx.wait_for_autosave(cx);
    assert!(fx.read("login.md").ends_with("Notes here. More.\n"));
    // The card face follows.
    cx.update(|cx| {
        let view = fx.view.read(cx);
        assert!(
            view.board
                .card(&key("login.md"))
                .unwrap()
                .excerpt
                .ends_with("More.")
        );
    });
}

#[gpui_kit::test]
fn the_title_and_the_first_heading_stay_the_same_thing(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.set_title(cx, "Fix the login page");
    fx.wait_for_autosave(cx);
    let after = fx.read("login.md");
    assert!(
        after.ends_with("---\n# Fix the login page\n\nNotes here.\n"),
        "{after}"
    );
    assert_eq!(changed_keys(CARD, &after), ["modified", "# Fix login"]);
    assert_eq!(fx.body(cx), "# Fix the login page\n\nNotes here.");

    // The other way round: retype the heading in the body, and the title field follows.
    fx.set_body(cx, "# Login is fixed\n\nNotes here.");
    fx.wait_for_autosave(cx);
    assert!(
        fx.read("login.md")
            .ends_with("---\n# Login is fixed\n\nNotes here.\n")
    );
    assert_eq!(fx.title(cx), "Login is fixed");
    // And nothing bounces back on the next save.
    fx.wait_for_autosave(cx);
    assert!(
        fx.read("login.md")
            .ends_with("---\n# Login is fixed\n\nNotes here.\n")
    );
}

#[gpui_kit::test]
fn field_edits_write_only_their_own_keys(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.retype(cx, |editor| editor.assignee.clone(), "dana");
    fx.retype(cx, |editor| editor.labels.clone(), "auth,  ui ,");
    fx.wait_for_autosave(cx);
    let after = fx.read("login.md");
    assert_eq!(
        changed_keys(CARD, &after),
        ["assignee", "modified", "labels"]
    );
    assert!(
        after.contains("assignee: \"dana\"\n") && after.contains("labels: [\"auth\", \"ui\"]\n")
    );
}

#[gpui_kit::test]
fn a_change_on_disk_reloads_a_clean_editor_silently(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    let by_agent = CARD
        .replace("Notes here.", "The agent wrote this.")
        .replace("\"todo\"", "\"review\"");
    fx.external_write(cx, "login.md", &by_agent);
    assert_eq!(fx.flags(cx), (false, false, false));
    assert_eq!(fx.body(cx), "# Fix login\n\nThe agent wrote this.");
    cx.update(|cx| {
        assert_eq!(
            fx.view.read(cx).editor.as_ref().unwrap().status,
            Status::Review
        )
    });
    fx.wait_for_autosave(cx);
    assert_eq!(fx.read("login.md"), by_agent, "a reload is not an edit");
}

#[gpui_kit::test]
fn a_change_on_disk_during_unsaved_typing_asks_first(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " My unsaved words.");
    cx.run_until_parked();
    let by_agent = CARD
        .replace("\"todo\"", "\"done\"")
        .replace("sprint: 4", "sprint: 9");
    fx.external_write(cx, "login.md", &by_agent);
    assert_eq!(fx.flags(cx), (true, false, false), "the clash banner shows");

    // Nothing is written while the question is open, however long it takes.
    fx.wait_for_autosave(cx);
    assert_eq!(fx.read("login.md"), by_agent);
    assert!(
        fx.body(cx).ends_with("My unsaved words."),
        "my text is still in the editor"
    );

    // Keep mine: my body goes on top of their file. Their status and keys stay.
    fx.with(cx, |view, window, cx| {
        view.resolve_conflict(true, window, cx)
    });
    fx.settle(cx);
    let after = fx.read("login.md");
    assert!(
        after.ends_with("Notes here. My unsaved words.\n"),
        "{after}"
    );
    assert!(after.contains("status: \"done\"\n") && after.contains("sprint: 9\n"));
    assert_eq!(fx.flags(cx), (false, false, false));
}

#[gpui_kit::test]
fn load_theirs_drops_my_unsaved_edit(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " Mine.");
    cx.run_until_parked();
    let by_agent = CARD.replace("Notes here.", "Theirs.");
    fx.external_write(cx, "login.md", &by_agent);
    assert!(fx.flags(cx).0);
    fx.with(cx, |view, window, cx| {
        view.resolve_conflict(false, window, cx)
    });
    fx.wait_for_autosave(cx);
    assert_eq!(fx.body(cx), "# Fix login\n\nTheirs.");
    assert_eq!(fx.read("login.md"), by_agent);
}

#[gpui_kit::test]
fn a_file_deleted_on_disk_can_be_recreated(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fs::remove_file(fx.path("login.md")).unwrap();
    fx.watcher_saw(
        cx,
        StoreEvent::Removed {
            rel_path: "login.md".into(),
        },
    );
    assert_eq!(
        fx.flags(cx),
        (false, true, false),
        "the deleted banner shows"
    );
    cx.update(|cx| assert!(fx.view.read(cx).editor.is_some(), "the sheet stays open"));

    fx.with(cx, |view, window, cx| view.recreate_card(window, cx));
    fx.settle(cx);
    assert_eq!(fx.read("login.md"), CARD);
    assert_eq!(fx.flags(cx), (false, false, false));
}

#[gpui_kit::test]
fn the_toolbar_formats_the_selection_as_one_undo_step(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    let select = |fx: &Fx, cx: &mut TestAppContext, range: std::ops::Range<usize>| {
        fx.with(cx, |view, window, cx| {
            let body = view.editor.as_ref().unwrap().body.clone();
            body.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_selected_range(range, cx);
            });
        })
    };
    // "Notes" in "# Fix login\n\nNotes here."
    select(&fx, cx, 13..18);
    fx.with(cx, |view, window, cx| {
        view.format(Tool::Wrap("**"), window, cx)
    });
    assert_eq!(fx.body(cx), "# Fix login\n\n**Notes** here.");
    cx.update(|cx| {
        let editor = fx.view.read(cx).editor.as_ref().unwrap();
        assert_eq!(
            editor.body.read(cx).selected_range(),
            15..20,
            "the word stays selected"
        );
    });
    // The same button takes the marks off again.
    fx.with(cx, |view, window, cx| {
        view.format(Tool::Wrap("**"), window, cx)
    });
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here.");

    // The keyboard shortcut does the same, and the editor's own undo reverts it in one step.
    let (bold, undo) = if cfg!(target_os = "macos") {
        ("cmd-b", "cmd-z")
    } else {
        ("ctrl-b", "ctrl-z")
    };
    select(&fx, cx, 13..18);
    cx.update_window(fx.window.into(), |_, window, cx| window.press(bold, cx))
        .unwrap();
    assert_eq!(fx.body(cx), "# Fix login\n\n**Notes** here.");
    cx.update_window(fx.window.into(), |_, window, cx| window.press(undo, cx))
        .unwrap();
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here.");

    // A block command lands on its own lines.
    select(&fx, cx, 24..24);
    fx.with(cx, |view, window, cx| view.format(Tool::Table, window, cx));
    assert!(
        fx.body(cx)
            .ends_with("Notes here.\n\n| Column | Column |\n| --- | --- |\n|  |  |")
    );
}

#[gpui_kit::test]
fn broken_frontmatter_is_fixed_in_the_raw_file(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "broken.md");
    assert_eq!(fx.flags(cx), (false, false, true), "raw mode");
    assert_eq!(fx.body(cx), BROKEN, "the editor holds the whole file");

    let fixed = "---\nstatus: \"review\"\n---\n# Broken\n";
    fx.set_body(cx, fixed);
    fx.wait_for_autosave(cx);
    assert_eq!(fx.read("broken.md"), fixed);
    // The form is back, with the fields read from the fixed file.
    assert_eq!(fx.flags(cx), (false, false, false));
    assert_eq!(fx.body(cx), "# Broken");
    cx.update(|cx| {
        assert_eq!(
            fx.view.read(cx).editor.as_ref().unwrap().status,
            Status::Review
        )
    });
}

#[gpui_kit::test]
fn switching_cards_saves_the_one_being_left(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " Left behind.");
    fx.open_card(cx, "other.md");
    assert!(fx.read("login.md").ends_with("Notes here. Left behind.\n"));
    assert_eq!(fx.body(cx), "# Other card\n\nOther notes.");
    fx.wait_for_autosave(cx);
    assert_eq!(
        fx.read("other.md"),
        OTHER,
        "the edit did not land in the wrong file"
    );
}

#[gpui_kit::test]
fn delete_asks_first_and_undo_brings_the_file_back(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.with(cx, |view, _, _| {
        view.confirm = Some(Confirm::Delete(key("login.md")))
    });
    fx.with(cx, |view, window, cx| view.answer_confirm(true, window, cx));
    fx.settle(cx);
    assert!(!fx.path("login.md").exists());
    cx.update(|cx| {
        let view = fx.view.read(cx);
        assert!(
            view.editor.is_none() && view.open_card.is_none(),
            "the sheet closes"
        );
    });
    fx.with(cx, |view, _, cx| view.undo(cx));
    assert_eq!(fx.read("login.md"), CARD);
}

#[gpui_kit::test]
fn the_ai_prompt_is_electrons(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    let prompt = cx.update(|cx| fx.view.read(cx).ai_prompt().unwrap());
    assert_eq!(
        prompt,
        "Work on this kanban task.\n\nFile: tasks/login.md\nTitle: Fix login\nStatus: todo\nPriority: high\nAssignee: safi\nLabels: auth\n\n# Fix login\n\nNotes here.\n\nRules: read the file, implement the work, then update the same markdown file (move status, update body checkboxes). Keep YAML frontmatter keys intact."
    );
    fx.open_card(cx, "other.md");
    let prompt = cx.update(|cx| fx.view.read(cx).ai_prompt().unwrap());
    assert!(prompt.contains("Assignee: unassigned\nLabels: none\n"));
}

#[gpui_kit::test]
fn escape_closes_the_sheet_and_saves(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " Then Escape.");
    cx.update_window(fx.window.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.update(|cx| assert!(fx.view.read(cx).editor.is_none()));
    assert!(fx.read("login.md").ends_with("Notes here. Then Escape.\n"));
}

#[gpui_kit::test]
fn enter_continues_a_list(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.set_body(cx, "# T\n\n- [ ] one");
    fx.type_in_body(cx, "");
    cx.update_window(fx.window.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.update_window(fx.window.into(), |_, window, cx| window.input("two", cx))
        .unwrap();
    assert_eq!(fx.body(cx), "# T\n\n- [ ] one\n- [ ] two");
}

#[allow(dead_code)]
fn unused(_: &Path) {}

// ---- M5: Vim keys in the card editor ---------------------------------------------------------

impl Fx {
    /// Open a card with Vim on, the keyboard in the body and the cursor at the start.
    fn open_with_vim(&self, cx: &mut TestAppContext, rel: &str) {
        cx.update(|cx| self.view.update(cx, |view, cx| view.set_vim_keys(true, cx)));
        self.open_card(cx, rel);
        self.with(cx, |view, window, cx| {
            let body = view.editor.as_ref().unwrap().body.clone();
            body.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_selected_range(0..0, cx);
            });
        });
    }

    /// Type printable keys, one keystroke each.
    fn keys(&self, cx: &mut TestAppContext, text: &str) {
        cx.update_window(self.window.into(), |_, window, cx| window.input(text, cx))
            .unwrap();
        cx.run_until_parked();
    }

    fn press(&self, cx: &mut TestAppContext, key: &str) {
        cx.update_window(self.window.into(), |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    }

    fn vim_status(&self, cx: &mut TestAppContext) -> String {
        cx.update(|cx| self.view.read(cx).editor.as_ref().unwrap().vim.status())
    }

    fn cursor(&self, cx: &mut TestAppContext) -> usize {
        cx.update(|cx| {
            self.view
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .body
                .read(cx)
                .cursor()
        })
    }

    fn is_open(&self, cx: &mut TestAppContext) -> bool {
        cx.update(|cx| self.view.read(cx).editor.is_some())
    }
}

// The body is "# Fix login\n\nNotes here."

#[gpui_kit::test]
fn vim_normal_mode_keys_move_and_edit_without_typing(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_with_vim(cx, "login.md");
    assert_eq!(fx.vim_status(cx), "-- NORMAL --");
    fx.keys(cx, "jj");
    assert_eq!(fx.cursor(cx), 13, "two lines down");
    assert_eq!(
        fx.body(cx),
        "# Fix login\n\nNotes here.",
        "normal mode keys are not text"
    );
    fx.keys(cx, "w");
    assert_eq!(fx.cursor(cx), 19);
    fx.keys(cx, "$");
    assert_eq!(
        fx.cursor(cx),
        23,
        "shifted keys reach Vim as their character"
    );
    fx.keys(cx, "gg");
    assert_eq!(fx.cursor(cx), 0);
    fx.keys(cx, "G");
    assert_eq!(fx.cursor(cx), 13);

    fx.keys(cx, "dd");
    assert_eq!(fx.body(cx), "# Fix login\n");
    fx.keys(cx, "u");
    assert_eq!(
        fx.body(cx),
        "# Fix login\n\nNotes here.",
        "u is the editor's undo"
    );
    // After an undo the cursor is where the editor's own history leaves it, so go back.
    fx.keys(cx, "ggjjdw");
    assert_eq!(fx.body(cx), "# Fix login\n\nhere.");
}

#[gpui_kit::test]
fn vim_insert_mode_types_and_escape_never_closes_the_card(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_with_vim(cx, "login.md");
    fx.keys(cx, "GA");
    assert_eq!(fx.vim_status(cx), "-- INSERT --");
    fx.keys(cx, " dd jj typed");
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here. dd jj typed");
    // Enter and Backspace are the editor's again while inserting.
    fx.press(cx, "enter");
    fx.keys(cx, "nextt");
    fx.press(cx, "backspace");
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here. dd jj typed\nnext");

    fx.press(cx, "escape");
    assert_eq!(fx.vim_status(cx), "-- NORMAL --");
    for _ in 0..3 {
        fx.press(cx, "escape");
    }
    assert!(
        fx.is_open(cx),
        "Escape never closes the card while Vim is on"
    );

    // The edit still autosaves like any other.
    fx.wait_for_autosave(cx);
    assert!(
        fx.read("login.md")
            .ends_with("Notes here. dd jj typed\nnext\n")
    );
}

#[gpui_kit::test]
fn vim_normal_mode_blocks_the_editors_own_keys(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_with_vim(cx, "login.md");
    fx.keys(cx, "jj");
    for key in ["enter", "backspace", "delete", "tab"] {
        fx.press(cx, key);
    }
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here.");
}

#[gpui_kit::test]
fn vim_visual_search_and_ex_commands(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_with_vim(cx, "login.md");
    fx.keys(cx, "jjve");
    assert_eq!(fx.vim_status(cx), "-- VISUAL --");
    cx.update(|cx| {
        let editor = fx.view.read(cx).editor.as_ref().unwrap();
        assert_eq!(
            editor.body.read(cx).selected_range(),
            13..18,
            "the word is selected"
        );
    });
    fx.keys(cx, "d");
    assert_eq!(fx.body(cx), "# Fix login\n\n here.");

    fx.keys(cx, "gg/here");
    assert_eq!(fx.vim_status(cx), "/here");
    fx.press(cx, "enter");
    assert_eq!(fx.cursor(cx), 14);

    // :w does nothing. :q closes the card, saving on the way out.
    fx.keys(cx, ":w");
    fx.press(cx, "enter");
    assert!(fx.is_open(cx));
    fx.keys(cx, ":q");
    fx.press(cx, "enter");
    assert!(!fx.is_open(cx));
    assert!(fx.read("login.md").ends_with("# Fix login\n\n here.\n"));
}

#[gpui_kit::test]
fn with_vim_off_the_editor_is_a_plain_editor(cx: &mut TestAppContext) {
    let fx = open(cx);
    fx.open_card(cx, "login.md");
    fx.type_in_body(cx, " jj dd");
    assert_eq!(fx.body(cx), "# Fix login\n\nNotes here. jj dd");
    fx.press(cx, "escape");
    assert!(!fx.is_open(cx), "Escape closes the card when Vim is off");
}
