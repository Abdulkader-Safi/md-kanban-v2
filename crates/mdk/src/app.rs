//! The root view. It owns the board and the preferences, and wires both to the window:
//! background scans, the file watcher, saving the config, and the single-instance signal.

use crate::board::{Board, CardKey, ProjectId};
use crate::prefs::{Prefs, SavedProject, ThemeChoice, WindowState};
use crate::theme;
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use mdk_format::{Date, Status};
use mdk_store::{Store, StoreError, StoreEvent, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

actions!(mdk, [Quit, CloseTop, ConnectFolder, RescanAll, ToggleTheme]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("escape", CloseTop, Some("Mdk")),
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
    pub(crate) lists: Vec<ListState>,
    pub(crate) columns: Vec<Vec<(CardKey, u64)>>,
    /// The card open in the side sheet.
    pub open_card: Option<CardKey>,
    /// The sidebar overlay, for windows too narrow to keep the sidebar on screen.
    pub(crate) show_sidebar: bool,
    /// Local time of the last refresh, for the "checked" line.
    pub(crate) last_check: Option<String>,
    /// One line of news for the user. Toasts replace this in M3.
    pub notice: Option<String>,
    save_task: Option<Task<()>>,
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
            show_sidebar: false,
            last_check: None,
            notice,
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
        if self.watchers.contains_key(id)
            || std::env::var_os("MDK_NO_WATCH").is_some_and(|v| v == "1")
        {
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

    // ---- actions the later milestones fill in -----------------------------------------------

    /// Open the New card dialog for a column. The dialog arrives in M3.
    pub fn new_card(&mut self, _status: Status, cx: &mut Context<Self>) {
        self.notice = Some("Creating cards arrives in the next milestone (M3).".into());
        cx.notify();
    }

    pub fn open(&mut self, key: CardKey, cx: &mut Context<Self>) {
        self.open_card = Some(key);
        cx.notify();
    }

    pub(crate) fn close_top(&mut self, _: &CloseTop, _: &mut Window, cx: &mut Context<Self>) {
        if self.open_card.take().is_none() {
            self.show_sidebar = false;
        }
        cx.notify();
    }
}
