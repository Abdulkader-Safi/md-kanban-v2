//! The root view. It owns the board and the preferences, and wires both to the window:
//! background scans, the file watcher, saving the config, and the single-instance signal.

use crate::board::{Board, CardKey, NewCard, ProjectId};
use crate::prefs::{Prefs, SavedProject, ThemeChoice, WindowState};
use crate::theme;
use crate::views::{CardEditor, NewCardForm};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui_kit::base::AutoScroll;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use mdk_format::{Date, Status};
use mdk_store::{Store, StoreError, StoreEvent, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

actions!(
    mdk,
    [
        Quit,
        CloseTop,
        Undo,
        Redo,
        NewCardKey,
        PickUp,
        OpenFocused,
        FocusUp,
        FocusDown,
        FocusLeft,
        FocusRight
    ]
);

/// "Mdk" is the whole window. "Board" is the board area only, so its plain keys (N, Space,
/// arrows) never fire while the user is typing in a dialog or the editor.
pub fn bind_keys(cx: &mut App) {
    crate::views::editor::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("escape", CloseTop, Some("Mdk")),
        KeyBinding::new("secondary-z", Undo, Some("Mdk")),
        KeyBinding::new("secondary-shift-z", Redo, Some("Mdk")),
        KeyBinding::new("n", NewCardKey, Some("Board")),
        KeyBinding::new("space", PickUp, Some("Board")),
        KeyBinding::new("enter", OpenFocused, Some("Board")),
        KeyBinding::new("up", FocusUp, Some("Board")),
        KeyBinding::new("down", FocusDown, Some("Board")),
        KeyBinding::new("left", FocusLeft, Some("Board")),
        KeyBinding::new("right", FocusRight, Some("Board")),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Today in the user's time zone.
pub fn today() -> Date {
    Date::parse(&chrono::Local::now().format("%Y-%m-%d").to_string()).expect("a valid date")
}

pub fn window_options(prefs: &Prefs, cx: &App) -> WindowOptions {
    let bounds = match prefs.window {
        Some(w) => Bounds::new(point(px(w.x), px(w.y)), size(px(w.w), px(w.h))),
        None => Bounds::centered(None, size(px(1440.), px(900.)), cx),
    };
    let maximized = prefs.window.is_some_and(|w| w.maximized);
    WindowOptions {
        window_bounds: Some(if maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }),
        titlebar: Some(TitlebarOptions {
            title: Some("mdk".into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(720.), px(480.))),
        ..Default::default()
    }
}

/// The other end of the single-instance signal: yields once each time mdk is launched again.
pub type Instance = UnboundedReceiver<()>;

/// Make this the one running mdk. Returns `None` when another mdk already runs: it has then
/// been asked to show its window, and this process should exit.
// ponytail: a Unix socket in the config folder. Windows has no check yet and can open twice;
// a named pipe goes in with the Windows packaging work (M8).
pub fn single_instance(config_dir: &Path) -> Option<Instance> {
    let (tx, rx) = unbounded();
    #[cfg(unix)]
    {
        use std::os::unix::net::{UnixListener, UnixStream};
        let _ = std::fs::create_dir_all(config_dir);
        let socket = config_dir.join("mdk.sock");
        if UnixStream::connect(&socket).is_ok() {
            return None;
        }
        // Nobody answered, so a socket file here is left over from a crash.
        let _ = std::fs::remove_file(&socket);
        if let Ok(listener) = UnixListener::bind(&socket) {
            std::thread::spawn(move || {
                for _ in listener.incoming() {
                    if tx.unbounded_send(()).is_err() {
                        break;
                    }
                }
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = config_dir;
        std::mem::forget(tx);
    }
    Some(rx)
}

/// A short message at the bottom of the window. It goes away after five seconds.
pub struct Toast {
    pub text: String,
    /// Show an Undo button.
    pub undo: bool,
}

/// A question the user must answer before something destructive happens.
pub enum Confirm {
    /// Archive this many done cards.
    Archive(usize),
    /// Delete this card's file.
    Delete(CardKey),
}

/// A card picked up with Space, and where it was, so Escape can put it back.
pub struct Grabbed {
    pub key: CardKey,
    status: Status,
    order: f64,
}

/// The payload of a card drag. It also draws the preview that follows the pointer.
#[derive(Clone)]
pub struct DragCard {
    pub key: CardKey,
    pub title: SharedString,
}

pub struct Mdk {
    pub board: Board,
    pub prefs: Prefs,
    config_dir: PathBuf,
    watchers: HashMap<ProjectId, Watcher>,
    /// False when live updates are switched off for the whole run (`MDK_NO_WATCH=1`, tests).
    watch_enabled: bool,
    events: UnboundedSender<(ProjectId, Vec<StoreEvent>)>,
    /// One virtualised list per column, and the cards each one currently shows. Every list
    /// has one row more than its cards: the last row is "+ Add card".
    pub lists: Vec<ListState>,
    pub(crate) columns: Vec<Vec<(CardKey, u64)>>,
    /// The card open in the side sheet, and the sheet's form.
    pub open_card: Option<CardKey>,
    pub editor: Option<CardEditor>,
    /// The sidebar overlay, for windows too narrow to keep the sidebar on screen.
    pub(crate) show_sidebar: bool,
    /// Local time of the last refresh, for the "checked" line.
    pub(crate) last_check: Option<String>,
    /// A problem that stays on screen until dismissed, such as a config that would not load.
    pub notice: Option<String>,
    pub toast: Option<Toast>,
    toast_task: Option<Task<()>>,
    pub confirm: Option<Confirm>,
    /// The New card dialog, when open.
    pub new_card: Option<NewCardForm>,
    /// The card the keyboard is on.
    pub focused: Option<CardKey>,
    pub grabbed: Option<Grabbed>,
    /// The card whose move menu is open.
    pub(crate) menu_for: Option<CardKey>,
    /// While dragging: the card in the air and where it would land, as (column, row).
    pub(crate) dragging: Option<CardKey>,
    pub slot: Option<(usize, usize)>,
    column_scroll: AutoScroll,
    board_auto_scroll: AutoScroll,
    scroll_column: usize,
    pub(crate) board_scroll: ScrollHandle,
    save_task: Option<Task<()>>,
    /// Focus for the board area, which owns the plain-key shortcuts.
    pub(crate) focus: FocusHandle,
}

impl Mdk {
    pub fn new(
        prefs: Prefs,
        config_dir: PathBuf,
        notice: Option<String>,
        mut instance: Instance,
        watch_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::apply(prefs.theme, window, cx);

        let mut board = Board::default();
        for p in &prefs.projects {
            board.add_project(p.id.clone(), p.name.clone(), p.root.clone());
        }
        let pick = |s: &str| (s != "all").then(|| s.to_string());
        board.select_project(pick(&prefs.selected_project));
        if board.selected_project.is_some() || prefs.selected_project == "all" {
            board.select_workspace(pick(&prefs.selected_workspace));
        }

        let (events, mut incoming) = unbounded::<(ProjectId, Vec<StoreEvent>)>();
        cx.spawn(async move |this, cx| {
            while let Some((id, batch)) = incoming.next().await {
                let alive = this.update(cx, |this, cx| this.on_store_events(id, batch, cx));
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        // A second launch of mdk brings this window forward.
        let handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            while instance.next().await.is_some() {
                cx.update(|cx| {
                    cx.activate(true);
                    let _ = cx.update_window(handle, |_, window, _| window.activate_window());
                });
            }
        })
        .detach();

        cx.observe_window_bounds(window, |this, window, cx| {
            this.remember_window(window);
            this.save_soon(cx);
        })
        .detach();
        cx.observe_window_appearance(window, |this, window, cx| {
            if this.prefs.theme == ThemeChoice::System {
                theme::apply(ThemeChoice::System, window, cx);
            }
        })
        .detach();
        // The OS does not always say when a watched folder goes. Look when the window comes back.
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.check_folders(cx);
            }
        })
        .detach();
        cx.on_app_quit(|this, _| {
            this.save_now();
            async {}
        })
        .detach();

        let mut this = Mdk {
            board,
            prefs,
            config_dir,
            watchers: HashMap::new(),
            watch_enabled,
            events,
            lists: Status::ALL
                .iter()
                .map(|_| ListState::new(1, ListAlignment::Top, px(600.)))
                .collect(),
            columns: vec![Vec::new(); Status::ALL.len()],
            open_card: None,
            editor: None,
            show_sidebar: false,
            last_check: None,
            notice,
            toast: None,
            toast_task: None,
            confirm: None,
            new_card: None,
            focused: None,
            grabbed: None,
            menu_for: None,
            dragging: None,
            slot: None,
            column_scroll: AutoScroll::default(),
            board_auto_scroll: AutoScroll::default(),
            scroll_column: 0,
            board_scroll: ScrollHandle::new(),
            save_task: None,
            focus: cx.focus_handle(),
        };
        this.focus.focus(window, cx);
        this.rescan_all(cx);
        this
    }

    // ---- scanning and live updates ----------------------------------------------------------

    /// Read a project from disk on a background thread, then show the result.
    pub fn rescan(&mut self, id: &str, cx: &mut Context<Self>) {
        let id = id.to_string();
        let Some(store) = self.board.store(&id) else {
            self.board
                .apply_scan(&id, Err(StoreError::NotFound(id.clone())));
            self.changed(cx);
            return;
        };
        cx.spawn(async move |this, cx| {
            let scanner = store.clone();
            let scan = cx
                .background_executor()
                .spawn(async move { scanner.scan() })
                .await;
            this.update(cx, |this, cx| {
                let ok = scan.is_ok();
                this.board.apply_scan(&id, scan);
                if ok {
                    this.watch(&id, &store);
                }
                this.checked();
                this.changed(cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn rescan_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<ProjectId> = self.board.projects.iter().map(|p| p.id.clone()).collect();
        for id in ids {
            self.rescan(&id, cx);
        }
    }

    fn watch(&mut self, id: &str, store: &Store) {
        if !self.watch_enabled || self.watchers.contains_key(id) {
            return;
        }
        let (tx, project) = (self.events.clone(), id.to_string());
        match store.watch(move |batch| drop(tx.unbounded_send((project.clone(), batch)))) {
            Ok(watcher) => drop(self.watchers.insert(id.to_string(), watcher)),
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn on_store_events(&mut self, id: ProjectId, batch: Vec<StoreEvent>, cx: &mut Context<Self>) {
        if !self.prefs.auto_refresh {
            return;
        }
        if self.board.apply_events(&id, batch) {
            self.rescan(&id, cx);
        }
        self.checked();
        self.changed(cx);
    }

    /// Rescan any project whose folder has gone, or has come back.
    fn check_folders(&mut self, cx: &mut Context<Self>) {
        let stale: Vec<ProjectId> = self
            .board
            .projects
            .iter()
            .filter(|p| p.root.is_dir() != (p.status == crate::board::ProjectStatus::Ok))
            .map(|p| p.id.clone())
            .collect();
        for id in stale {
            self.rescan(&id, cx);
        }
    }

    /// Stop every file watcher. Live updates stay off until the next rescan.
    pub fn stop_watching(&mut self) {
        self.watchers.clear();
    }

    fn checked(&mut self) {
        self.last_check = Some(chrono::Local::now().format("%H:%M:%S").to_string());
    }

    /// The board changed: bring the column lists in line and redraw.
    pub fn changed(&mut self, cx: &mut Context<Self>) {
        // A watcher on a folder that went missing is dead. Drop it, so a rescan starts a new one.
        let board = &self.board;
        self.watchers.retain(|id, _| {
            board
                .project(id)
                .is_some_and(|p| p.status == crate::board::ProjectStatus::Ok)
        });
        if self
            .open_card
            .as_ref()
            .is_some_and(|key| self.board.card(key).is_none())
        {
            self.open_card = None;
        }
        self.sync_lists();
        cx.notify();
    }

    /// Tell each column's list which rows changed, so it keeps its scroll position.
    fn sync_lists(&mut self) {
        for (i, status) in Status::ALL.into_iter().enumerate() {
            let new: Vec<(CardKey, u64)> = self
                .board
                .column(status)
                .iter()
                .map(|card| (card.key.clone(), card.content_hash))
                .collect();
            let old = &self.columns[i];
            if *old == new {
                continue;
            }
            let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(new[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            self.lists[i].splice(prefix..old.len() - suffix, new.len() - prefix - suffix);
            self.columns[i] = new;
        }
    }

    // ---- projects ---------------------------------------------------------------------------

    /// Ask for a folder with the OS picker, then hand it to `then`.
    fn pick_folder(
        &mut self,
        prompt: &'static str,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &Path, &mut Context<Self>) + 'static,
    ) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.first()
            {
                this.update(cx, |this, cx| then(this, path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn connect_folder(&mut self, cx: &mut Context<Self>) {
        self.pick_folder("Connect", cx, |this, path, cx| this.connect(path, cx));
    }

    pub fn connect(&mut self, path: &Path, cx: &mut Context<Self>) {
        match self.board.connect_project(path) {
            Ok(id) => {
                self.notice = None;
                self.rescan(&id, cx);
            }
            Err(error) => self.notice = Some(error),
        }
        self.selection_changed(cx);
    }

    pub fn locate_folder(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        self.pick_folder("Locate", cx, move |this, path, cx| {
            match this.board.relocate_project(&id, path) {
                Ok(()) => this.rescan(&id, cx),
                Err(error) => this.notice = Some(error),
            }
            this.selection_changed(cx);
        });
    }

    pub fn remove_project(&mut self, id: &str, cx: &mut Context<Self>) {
        self.board.remove_project(id);
        self.watchers.remove(id);
        self.selection_changed(cx);
    }

    pub fn select_project(&mut self, id: Option<ProjectId>, cx: &mut Context<Self>) {
        self.board.select_project(id);
        self.selection_changed(cx);
    }

    pub fn select_workspace(&mut self, workspace: Option<String>, cx: &mut Context<Self>) {
        self.board.select_workspace(workspace);
        self.selection_changed(cx);
    }

    /// The project list or the selection changed: save it and redraw.
    fn selection_changed(&mut self, cx: &mut Context<Self>) {
        self.save_soon(cx);
        self.changed(cx);
    }

    // ---- preferences ------------------------------------------------------------------------

    pub fn set_auto_refresh(&mut self, on: bool, cx: &mut Context<Self>) {
        self.prefs.auto_refresh = on;
        if on {
            // Catch up on whatever changed while updates were paused.
            self.rescan_all(cx);
        }
        self.save_soon(cx);
        cx.notify();
    }

    /// A preference changed: save it and redraw.
    pub(crate) fn prefs_changed(&mut self, cx: &mut Context<Self>) {
        self.save_soon(cx);
        cx.notify();
    }

    pub fn set_vim_keys(&mut self, on: bool, cx: &mut Context<Self>) {
        self.prefs.vim_keys = on;
        self.save_soon(cx);
        cx.notify();
    }

    pub fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.theme = if cx.theme().is_dark() {
            ThemeChoice::Light
        } else {
            ThemeChoice::Dark
        };
        theme::apply(self.prefs.theme, window, cx);
        self.save_soon(cx);
        cx.notify();
    }

    fn remember_window(&mut self, window: &Window) {
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) => (bounds, false),
            WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => (bounds, true),
        };
        self.prefs.window = Some(WindowState {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            w: bounds.size.width.into(),
            h: bounds.size.height.into(),
            maximized,
        });
    }

    /// Write the config 300 ms from now. A newer change restarts the wait.
    fn save_soon(&mut self, cx: &mut Context<Self>) {
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            this.update(cx, |this, _| this.save_now()).ok();
        }));
    }

    pub fn save_now(&mut self) {
        let all = || "all".to_string();
        self.prefs.projects = self
            .board
            .projects
            .iter()
            .map(|p| SavedProject {
                id: p.id.clone(),
                name: p.name.clone(),
                root: p.root.clone(),
            })
            .collect();
        self.prefs.selected_project = self.board.selected_project.clone().unwrap_or_else(all);
        self.prefs.selected_workspace = self.board.selected_workspace.clone().unwrap_or_else(all);
        if let Err(error) = self.prefs.save(&self.config_dir) {
            self.notice = Some(format!("Settings could not be saved: {error}"));
        }
    }

    // ---- toasts ----------------------------------------------------------------------------

    pub fn show_toast(&mut self, text: impl Into<String>, undo: bool, cx: &mut Context<Self>) {
        self.toast = Some(Toast {
            text: text.into(),
            undo,
        });
        self.toast_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Show the outcome of a board action: an error as a toast, and the board redrawn.
    fn settle<T>(&mut self, result: Result<T, String>, cx: &mut Context<Self>) -> Option<T> {
        let value = match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.show_toast(error, false, cx);
                None
            }
        };
        self.changed(cx);
        value
    }

    // ---- moving -----------------------------------------------------------------------------

    /// Move a card and offer Undo. `index` counts the column's other cards; `None` is the end.
    pub fn move_card(
        &mut self,
        key: &CardKey,
        status: Status,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.menu_for = None;
        let moved = self.board.move_card(key, status, index);
        if self.settle(moved, cx) == Some(true) {
            self.show_toast(format!("Moved to {}", status.name()), true, cx);
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let undone = self.board.undo();
        if let Some(Some(label)) = self.settle(undone, cx) {
            self.show_toast(format!("Undid: {label}"), false, cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        let redone = self.board.redo();
        if let Some(Some(label)) = self.settle(redone, cx) {
            self.show_toast(format!("Redid: {label}"), false, cx);
        }
    }

    /// Ask before archiving. Nothing happens when there are no done cards on show.
    pub fn ask_archive(&mut self, cx: &mut Context<Self>) {
        let count = self.board.archivable().len();
        if count > 0 {
            self.confirm = Some(Confirm::Archive(count));
            cx.notify();
        }
    }

    pub fn answer_confirm(&mut self, yes: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        if yes {
            match confirm {
                Confirm::Archive(_) => {
                    let archived = self.board.archive_done();
                    if let Some(count) = self.settle(archived, cx) {
                        self.show_toast(format!("Archived {count} cards to archive/"), true, cx);
                    }
                }
                Confirm::Delete(key) => {
                    // Close without saving: the file is about to go.
                    self.editor = None;
                    self.open_card = None;
                    let deleted = self.board.delete_card(&key);
                    if self.settle(deleted, cx).is_some() {
                        self.show_toast("Deleted", true, cx);
                    }
                }
            }
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    // ---- drag and drop ----------------------------------------------------------------------

    /// The pointer moved during a drag. Every column hears this, so check the bounds.
    pub(crate) fn drag_move(
        &mut self,
        column: usize,
        ev: &DragMoveEvent<DragCard>,
        cx: &mut Context<Self>,
    ) {
        let pos = ev.event.position;
        if !ev.bounds.contains(&pos) {
            if self.slot.is_some_and(|(c, _)| c == column) {
                self.slot = None;
                self.column_scroll.set(None, cx, |_, _: &mut Self, _| {});
                cx.notify();
            }
            return;
        }
        let list = &self.lists[column];
        let len = self.columns[column].len();
        // The first card whose middle is below the pointer. No such card means the end.
        let mut row = len;
        for i in list.logical_scroll_top().item_ix..len {
            match list.bounds_for_item(i) {
                Some(bounds) if pos.y >= bounds.center().y => {}
                _ => {
                    row = i;
                    break;
                }
            }
        }
        if self.slot != Some((column, row)) {
            self.slot = Some((column, row));
            cx.notify();
        }
        self.scroll_column = column;
        let delta = AutoScroll::compute_delta(pos.y, list.viewport_bounds());
        self.column_scroll.set(delta, cx, |d, this: &mut Self, cx| {
            this.lists[this.scroll_column].scroll_by(d);
            cx.notify();
        });
    }

    /// Scroll the board sideways while a drag is near its left or right edge.
    pub(crate) fn board_drag_move(&mut self, ev: &DragMoveEvent<DragCard>, cx: &mut Context<Self>) {
        // compute_delta works on y, so hand it the board's x range as if it were vertical.
        let sideways = Bounds::new(
            point(px(0.), ev.bounds.left()),
            size(px(0.), ev.bounds.size.width),
        );
        let delta = AutoScroll::compute_delta(ev.event.position.x, sideways);
        self.board_auto_scroll
            .set(delta, cx, |d, this: &mut Self, cx| {
                let (offset, max) = (this.board_scroll.offset(), this.board_scroll.max_offset());
                let x = (offset.x - d).clamp(-max.x, px(0.));
                this.board_scroll.set_offset(point(x, offset.y));
                cx.notify();
            });
    }

    pub(crate) fn drop_card(&mut self, column: usize, drag: &DragCard, cx: &mut Context<Self>) {
        let status = Status::ALL[column];
        let cards = &self.columns[column];
        let mut row = match self.slot {
            Some((c, row)) if c == column => row,
            _ => cards.len(),
        };
        let current = cards.iter().position(|(key, _)| *key == drag.key);
        self.end_drag();
        // The slot counts the dragged card too. Dropping it where it is moves nothing.
        if let Some(current) = current {
            if row == current || row == current + 1 {
                cx.notify();
                return;
            }
            if current < row {
                row -= 1;
            }
        }
        self.move_card(&drag.key.clone(), status, Some(row), cx);
    }

    pub(crate) fn end_drag(&mut self) {
        self.slot = None;
        self.dragging = None;
        self.column_scroll.stop();
        self.board_auto_scroll.stop();
    }

    // ---- keyboard: card focus and moving ----------------------------------------------------

    fn place_of(&self, key: &CardKey) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(c, cards)| cards.iter().position(|(k, _)| k == key).map(|row| (c, row)))
    }

    fn focus_card(&mut self, column: usize, row: usize, cx: &mut Context<Self>) {
        if let Some((key, _)) = self.columns[column].get(row) {
            self.focused = Some(key.clone());
            self.lists[column].scroll_to_reveal_item(row);
            cx.notify();
        }
    }

    /// An arrow key. With a card picked up it moves the card, otherwise it moves the focus.
    /// `dx` and `dy` are -1, 0 or 1.
    pub(crate) fn arrow(&mut self, dx: isize, dy: isize, cx: &mut Context<Self>) {
        let place = self.focused.as_ref().and_then(|key| self.place_of(key));
        let Some((column, row)) = place else {
            // Nothing focused yet: start at the top of the first column that has cards.
            if let Some(first) = self.columns.iter().position(|cards| !cards.is_empty()) {
                self.focus_card(first, 0, cx);
            }
            return;
        };
        if let Some(key) = self.grabbed.as_ref().map(|g| g.key.clone()) {
            let last = self.columns[column].len() - 1;
            if dx != 0 {
                if let Some(status) = Status::ALL.get(column.wrapping_add_signed(dx)) {
                    self.move_card(&key, *status, None, cx);
                }
            } else if (dy < 0 && row > 0) || (dy > 0 && row < last) {
                self.move_card(
                    &key,
                    Status::ALL[column],
                    Some(row.wrapping_add_signed(dy)),
                    cx,
                );
            }
            if let Some((column, row)) = self.place_of(&key) {
                self.lists[column].scroll_to_reveal_item(row);
            }
            return;
        }
        if dx != 0 {
            // The nearest column in that direction that has cards.
            let mut next = column.wrapping_add_signed(dx);
            while self.columns.get(next).is_some_and(Vec::is_empty) {
                next = next.wrapping_add_signed(dx);
            }
            if let Some(cards) = self.columns.get(next) {
                self.focus_card(next, row.min(cards.len() - 1), cx);
            }
        } else {
            let last = self.columns[column].len() - 1;
            self.focus_card(column, row.saturating_add_signed(dy).min(last), cx);
        }
    }

    /// Space: pick up the focused card, or drop the one in hand.
    pub(crate) fn pick_up_or_drop(&mut self, cx: &mut Context<Self>) {
        if self.grabbed.take().is_none()
            && let Some(card) = self.focused.as_ref().and_then(|key| self.board.card(key))
        {
            if card.error().is_some() {
                self.show_toast("Fix the frontmatter first", false, cx);
            } else {
                self.grabbed = Some(Grabbed {
                    key: card.key.clone(),
                    status: card.data.status,
                    order: card.data.order,
                });
            }
        }
        cx.notify();
    }

    /// Escape while a card is picked up: put it back where it was.
    fn cancel_grab(&mut self, cx: &mut Context<Self>) {
        let Some(grabbed) = self.grabbed.take() else {
            return;
        };
        let moved = self
            .board
            .card(&grabbed.key)
            .is_some_and(|c| c.data.status != grabbed.status || c.data.order != grabbed.order);
        if moved {
            let patch = mdk_format::CardPatch {
                status: Some(grabbed.status),
                order: Some(grabbed.order),
                ..Default::default()
            };
            let restored = self.board.update_card(&grabbed.key, &patch);
            self.settle(restored, cx);
        }
        self.toast = None;
        cx.notify();
    }

    // ---- creating ---------------------------------------------------------------------------

    /// Open the New card dialog for a column.
    pub fn open_new_card(&mut self, status: Status, window: &mut Window, cx: &mut Context<Self>) {
        if self.board.projects.is_empty() {
            self.show_toast("Connect a folder first", false, cx);
            return;
        }
        let folder = self.board.selected_workspace.clone().unwrap_or_default();
        let form = NewCardForm::new(status, &folder, window, cx);
        form.title.update(cx, |title, cx| title.focus(window, cx));
        self.new_card = Some(form);
        cx.notify();
    }

    pub fn close_new_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_card = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Create a card. Returns its key, or `None` after showing why it failed.
    pub fn create_card(
        &mut self,
        input: NewCard,
        workspace: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Option<CardKey> {
        let created = self.board.create_card(input, workspace, today());
        let key = self.settle(created, cx)?;
        self.focused = Some(key.clone());
        Some(key)
    }

    // ---- opening and closing ----------------------------------------------------------------

    /// Escape closes the top thing: a confirm, the dialog, a picked-up card, the menu, the
    /// card sheet (saving first), then the sidebar overlay.
    pub(crate) fn close_top(&mut self, _: &CloseTop, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm.is_some() {
            self.answer_confirm(false, window, cx);
        } else if self.new_card.is_some() {
            self.close_new_card(window, cx);
        } else if self.grabbed.is_some() {
            self.cancel_grab(cx);
        } else if self.menu_for.take().is_some() {
        } else if self.editor.is_some() {
            self.close_editor(window, cx);
        } else {
            self.show_sidebar = false;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
}
