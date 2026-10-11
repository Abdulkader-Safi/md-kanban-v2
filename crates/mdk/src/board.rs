//! The in-memory board: projects, their cards, and what is selected.
//!
//! No GPUI in here. The root view owns one `Board`, and every change to a card file
//! goes through a `Board` method, so there is a single writer (build spec, principle 2).

use mdk_format::{CardPatch, Date, LineEnding, ParsedCard, Priority, Status};
use mdk_store::{ScannedFile, Store, StoreError, StoreEvent, content_hash};
use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type ProjectId = String;

/// A card's identity: its project and its path. Never the frontmatter `id`, which can repeat.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardKey {
    pub project: ProjectId,
    pub rel_path: String,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub key: CardKey,
    pub data: ParsedCard,
    /// First path segment, empty for a card at the project root.
    pub workspace: String,
    /// The exact file text last read.
    pub raw: Arc<str>,
    pub content_hash: u64,
    pub excerpt: String,
    /// Checkboxes in the body: (done, total).
    pub subtasks: (usize, usize),
    /// Lowercased title, body, id, assignee and labels, for search.
    pub search_text: String,
    /// Why the file could not be read at all (too large, not UTF-8).
    pub unreadable: Option<String>,
}

impl Card {
    fn new(project: &str, rel_path: &str, content: Result<String, String>) -> Card {
        let file_name = rel_path.rsplit('/').next().unwrap_or(rel_path);
        let (raw, unreadable) = match content {
            Ok(text) => (text, None),
            Err(why) => (String::new(), Some(why)),
        };
        let data = mdk_format::parse(&raw, file_name);
        let subtasks = mdk_format::subtasks(&data.body);
        let search_text = format!(
            "{} {} {} {} {}",
            data.title,
            data.body,
            data.id,
            data.assignee,
            data.labels.join(" ")
        )
        .to_lowercase();
        Card {
            key: CardKey {
                project: project.to_string(),
                rel_path: rel_path.to_string(),
            },
            workspace: match rel_path.split_once('/') {
                Some((first, _)) => first.to_string(),
                None => String::new(),
            },
            content_hash: content_hash(&raw),
            excerpt: mdk_format::excerpt(&data.body),
            subtasks: (subtasks.iter().filter(|s| s.done).count(), subtasks.len()),
            search_text,
            raw: raw.into(),
            unreadable,
            data,
        }
    }

    /// The reason this card cannot be moved or rewritten, if there is one.
    pub fn error(&self) -> Option<&str> {
        self.unreadable
            .as_deref()
            .or(self.data.parse_error.as_deref())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProjectStatus {
    Ok,
    /// The folder is gone: moved, deleted, or its drive is unplugged.
    Missing,
    Unreadable(String),
}

pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
    pub status: ProjectStatus,
    /// True until the first scan comes back.
    pub loading: bool,
    store: Option<Store>,
}

impl Project {
    fn open(id: ProjectId, name: String, root: PathBuf) -> Project {
        let (store, status) = match Store::open(&root) {
            Ok(store) => (Some(store), ProjectStatus::Ok),
            Err(e) => (None, status_for(&root, &e)),
        };
        Project {
            id,
            name,
            loading: store.is_some(),
            root,
            status,
            store,
        }
    }
}

fn status_for(root: &Path, error: &StoreError) -> ProjectStatus {
    if root.is_dir() {
        ProjectStatus::Unreadable(error.to_string())
    } else {
        ProjectStatus::Missing
    }
}

/// A random 8-character base36 id, like Electron's.
fn new_id(seed: &Path) -> ProjectId {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    std::time::SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    let mut n = hasher.finish();
    (0..8)
        .map(|_| {
            let digit = std::char::from_digit((n % 36) as u32, 36).unwrap();
            n /= 36;
            digit
        })
        .collect()
}

#[derive(Default)]
pub struct Board {
    pub projects: Vec<Project>,
    cards: HashMap<CardKey, Card>,
    /// `None` is "All projects".
    pub selected_project: Option<ProjectId>,
    /// `None` is "all" work folders.
    pub selected_workspace: Option<String>,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
}

/// One change to one file, recorded so it can be undone. `None` means the file is absent.
#[derive(Clone, Debug, PartialEq)]
enum FileOp {
    Change {
        key: CardKey,
        before: Option<String>,
        after: Option<String>,
    },
    Rename {
        from: CardKey,
        to: CardKey,
    },
}

/// A board action and the file changes it made.
#[derive(Clone, Debug)]
struct UndoEntry {
    label: String,
    ops: Vec<FileOp>,
}

/// What the New card dialog collects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewCard {
    pub title: String,
    pub body: String,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub assignee: String,
    pub due_date: Option<String>,
    pub labels: Vec<String>,
}

const BROKEN: &str = "Fix the frontmatter first";

impl Board {
    // ---- projects ---------------------------------------------------------------------------

    /// Add a project that is already known (from the config file). Does not scan.
    pub fn add_project(&mut self, id: ProjectId, name: String, root: PathBuf) {
        self.projects.push(Project::open(id, name, root));
    }

    /// Connect a folder and select it. Connecting a folder twice selects the first one.
    /// Does not scan: the caller scans, in the background.
    pub fn connect_project(&mut self, path: &Path) -> Result<ProjectId, String> {
        let root = path
            .canonicalize()
            .map_err(|e| format!("Can't open {}: {e}", path.display()))?;
        if !root.is_dir() {
            return Err(format!("{} is not a folder", root.display()));
        }
        let id = match self.projects.iter().find(|p| p.root == root) {
            Some(existing) => existing.id.clone(),
            None => {
                let name = root.file_name().map_or_else(
                    || root.display().to_string(),
                    |n| n.to_string_lossy().into(),
                );
                let id = new_id(&root);
                self.add_project(id.clone(), name, root);
                id
            }
        };
        self.select_project(Some(id.clone()));
        Ok(id)
    }

    /// Forget a project. Its files are not touched.
    pub fn remove_project(&mut self, id: &str) {
        self.projects.retain(|p| p.id != id);
        self.cards.retain(|key, _| key.project != id);
        if self.selected_project.as_deref() == Some(id) {
            self.select_project(None);
        }
    }

    /// Point a project whose folder went missing at its new place. The caller then scans.
    pub fn relocate_project(&mut self, id: &str, path: &Path) -> Result<(), String> {
        let root = path
            .canonicalize()
            .map_err(|e| format!("Can't open {}: {e}", path.display()))?;
        let project = self.project_mut(id).ok_or("No such project")?;
        *project = Project::open(project.id.clone(), project.name.clone(), root);
        Ok(())
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    fn project_mut(&mut self, id: &str) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// The project's store, to scan or watch on another thread. Reopens a store that was
    /// lost when the folder went missing, in case the folder is back.
    pub fn store(&mut self, id: &str) -> Option<Store> {
        let project = self.project_mut(id)?;
        if project.store.is_none() {
            project.store = Store::open(&project.root).ok();
        }
        project.store.clone()
    }

    /// Take the result of a scan. A failed scan keeps the cards last read, as stale.
    pub fn apply_scan(&mut self, id: &str, scan: Result<Vec<ScannedFile>, StoreError>) {
        let Some(project) = self.project_mut(id) else {
            return;
        };
        project.loading = false;
        match scan {
            Ok(files) => {
                project.status = ProjectStatus::Ok;
                self.cards.retain(|key, _| key.project != id);
                for file in files {
                    let card = Card::new(id, &file.rel_path, file.content);
                    self.cards.insert(card.key.clone(), card);
                }
            }
            Err(error) => {
                project.status = status_for(&project.root, &error);
                project.store = None;
            }
        }
    }

    /// Scan a project right now, on this thread. For tests and small one-off reads.
    pub fn scan_now(&mut self, id: &str) {
        let scan = match self.store(id) {
            Some(store) => store.scan(),
            None => Err(StoreError::NotFound(id.to_string())),
        };
        self.apply_scan(id, scan);
    }

    /// Take a batch of watcher events. Returns true when the project needs a full scan.
    pub fn apply_events(&mut self, id: &str, events: Vec<StoreEvent>) -> bool {
        let mut rescan = false;
        for event in events {
            match event {
                StoreEvent::Changed { rel_path } => {
                    let Some(store) = self.store(id) else {
                        continue;
                    };
                    match store.read(&rel_path) {
                        Ok(text) => self.put_card(id, &rel_path, Ok(text)),
                        Err(StoreError::Rejected(why)) => self.put_card(id, &rel_path, Err(why)),
                        // Gone again already. A Removed event follows.
                        Err(_) => {}
                    }
                }
                StoreEvent::Removed { rel_path } => {
                    self.cards.remove(&CardKey {
                        project: id.to_string(),
                        rel_path,
                    });
                }
                StoreEvent::Rescan => rescan = true,
                StoreEvent::RootMissing => {
                    if let Some(project) = self.project_mut(id) {
                        project.status = ProjectStatus::Missing;
                        project.store = None;
                    }
                }
                // ponytail: watcher errors are dropped until logging lands. The next rescan recovers.
                StoreEvent::Error { .. } => {}
            }
        }
        rescan
    }

    fn put_card(&mut self, project: &str, rel_path: &str, content: Result<String, String>) {
        let card = Card::new(project, rel_path, content);
        self.cards.insert(card.key.clone(), card);
    }

    // ---- writing cards ----------------------------------------------------------------------
    //
    // Every change to a card file is made here. Each one re-reads the file, changes only
    // what the action names, writes atomically, and records how to undo itself.

    fn store_for(&mut self, key: &CardKey) -> Result<Store, String> {
        self.store(&key.project)
            .ok_or_else(|| "The project folder is not available".to_string())
    }

    /// Bring one card in memory in line with its file.
    fn reload(&mut self, key: &CardKey, text: Option<&str>) {
        match text {
            Some(text) => self.put_card(&key.project, &key.rel_path, Ok(text.to_string())),
            None => drop(self.cards.remove(key)),
        }
    }

    fn record(&mut self, label: impl Into<String>, ops: Vec<FileOp>) {
        if !ops.is_empty() {
            self.undo.push(UndoEntry {
                label: label.into(),
                ops,
            });
            self.redo.clear();
        }
    }

    /// Patch one card file on top of what is on disk right now.
    fn patch(&mut self, key: &CardKey, patch: &CardPatch) -> Result<FileOp, String> {
        if self.card(key).is_some_and(|c| c.error().is_some()) {
            return Err(BROKEN.to_string());
        }
        let store = self.store_for(key)?;
        let file_name = key.rel_path.rsplit('/').next().unwrap_or(&key.rel_path);
        let now = mdk_format::now_iso();
        let (before, after) = store
            .update(&key.rel_path, |raw| {
                mdk_format::apply_patch(raw, file_name, patch, &now).map_err(|e| e.to_string())
            })
            .map_err(|e| e.to_string())?;
        self.reload(key, Some(&after));
        Ok(FileOp::Change {
            key: key.clone(),
            before: Some(before),
            after: Some(after),
        })
    }

    /// Change a card's fields. Only the fields the patch names are written.
    pub fn update_card(&mut self, key: &CardKey, patch: &CardPatch) -> Result<(), String> {
        let op = self.patch(key, patch)?;
        let title = self
            .card(key)
            .map_or_else(String::new, |c| c.data.title.clone());
        self.record(format!("Edit {title}"), vec![op]);
        Ok(())
    }

    /// Move a card to a column, at `index` among the cards that column shows, or at the end.
    /// Writes only `status`, `order` and `modified`. Returns false when nothing had to move.
    pub fn move_card(
        &mut self,
        key: &CardKey,
        status: Status,
        index: Option<usize>,
    ) -> Result<bool, String> {
        let card = self.card(key).ok_or("That card is gone")?;
        if card.error().is_some() {
            return Err(BROKEN.to_string());
        }
        let (from_status, from_order) = (card.data.status, card.data.order);
        // The neighbours are the cards the user sees in that column, without the moving card.
        let column: Vec<(CardKey, f64)> = self
            .column(status)
            .into_iter()
            .filter(|c| c.key != *key)
            .map(|c| (c.key.clone(), c.data.order))
            .collect();

        let mut ops = Vec::new();
        let order = match index.filter(|&i| i < column.len()) {
            None => {
                // Already the last card of that column: nothing to do.
                if self
                    .column(status)
                    .last()
                    .is_some_and(|last| last.key == *key)
                {
                    return Ok(false);
                }
                match column.iter().map(|(_, order)| *order).reduce(f64::max) {
                    Some(max) => max.max(0.0) + 1.0,
                    None => 0.0,
                }
            }
            Some(i) => {
                let order = if i == 0 {
                    column[0].1 - 1.0
                } else {
                    (column[i - 1].1 + column[i].1) / 2.0
                };
                let collides = !order.is_finite()
                    || column[i].1 == order
                    || (i > 0 && column[i - 1].1 == order);
                if collides {
                    // No room between the neighbours. Renumber the column 0, 10, 20 and so on.
                    for (n, (neighbour, old)) in column.iter().enumerate() {
                        let new = (n * 10) as f64;
                        if *old != new {
                            let patch = CardPatch {
                                order: Some(new),
                                ..Default::default()
                            };
                            ops.push(self.patch(neighbour, &patch)?);
                        }
                    }
                    (i * 10) as f64 - 5.0
                } else {
                    order
                }
            }
        };
        if from_status == status && from_order == order && ops.is_empty() {
            return Ok(false);
        }
        let patch = CardPatch {
            status: Some(status),
            order: Some(order),
            ..Default::default()
        };
        ops.push(self.patch(key, &patch)?);
        self.record(format!("Move to {}", status.name()), ops);
        Ok(true)
    }

    /// Create a card file in the selected project, or the first one. `workspace` overrides
    /// the selected work folder. `today` dates the file name.
    pub fn create_card(
        &mut self,
        input: NewCard,
        workspace: Option<&str>,
        today: Date,
    ) -> Result<CardKey, String> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err("A card needs a title".to_string());
        }
        let project = self
            .selected_project
            .clone()
            .or_else(|| self.projects.first().map(|p| p.id.clone()))
            .ok_or("Connect a folder first")?;
        let folder = match workspace {
            Some(folder) => folder.trim().trim_matches('/').to_string(),
            None => self.selected_workspace.clone().unwrap_or_default(),
        };
        let store = self
            .store(&project)
            .ok_or("The project folder is not available")?;
        let rel_path = store
            .free_name(&folder, &mdk_format::new_file_name(title, today))
            .map_err(|e| e.to_string())?;
        let file_name = rel_path.rsplit('/').next().unwrap_or(&rel_path);
        let now = mdk_format::now_iso();
        let body = input.body.trim();
        let card = ParsedCard {
            // The file name is unique in its folder, so the id is too.
            id: file_name.trim_end_matches(".md").to_string(),
            title: title.to_string(),
            body: if body.is_empty() {
                format!("# {title}")
            } else {
                body.to_string()
            },
            status: input.status.unwrap_or(Status::Backlog),
            priority: input.priority.unwrap_or(Priority::Medium),
            assignee: input.assignee.trim().to_string(),
            due_date: input.due_date.filter(|d| !d.is_empty()),
            created: now.clone(),
            modified: now.clone(),
            labels: input.labels,
            // New cards go last: the time in milliseconds, as Electron does.
            order: chrono::Utc::now().timestamp_millis() as f64,
            extra: Vec::new(),
            line_ending: LineEnding::Lf,
            parse_error: None,
        };
        let text = mdk_format::write(&card, &now);
        store.write(&rel_path, &text).map_err(|e| e.to_string())?;
        let key = CardKey { project, rel_path };
        self.reload(&key, Some(&text));
        self.record(
            format!("Create {title}"),
            vec![FileOp::Change {
                key: key.clone(),
                before: None,
                after: Some(text),
            }],
        );
        Ok(key)
    }

    /// Delete a card file. Its exact text is kept, so the delete can be undone.
    pub fn delete_card(&mut self, key: &CardKey) -> Result<(), String> {
        let title = self
            .card(key)
            .map_or_else(String::new, |c| c.data.title.clone());
        let text = self
            .store_for(key)?
            .delete(&key.rel_path)
            .map_err(|e| e.to_string())?;
        self.reload(key, None);
        self.record(
            format!("Delete {title}"),
            vec![FileOp::Change {
                key: key.clone(),
                before: Some(text),
                after: None,
            }],
        );
        Ok(())
    }

    /// Replace a card file's whole text. This is how a file with broken frontmatter gets
    /// fixed: by hand, in the editor. A file that is gone is created again.
    pub fn write_raw(&mut self, key: &CardKey, text: &str) -> Result<(), String> {
        let store = self.store_for(key)?;
        let before = store.read(&key.rel_path).ok();
        if before.as_deref() == Some(text) {
            return Ok(());
        }
        store
            .write(&key.rel_path, text)
            .map_err(|e| e.to_string())?;
        self.reload(key, Some(text));
        let title = self
            .card(key)
            .map_or_else(String::new, |c| c.data.title.clone());
        let verb = if before.is_some() { "Edit" } else { "Recreate" };
        self.record(
            format!("{verb} {title}"),
            vec![FileOp::Change {
                key: key.clone(),
                before,
                after: Some(text.to_string()),
            }],
        );
        Ok(())
    }

    /// The done cards on show that are not archived yet.
    pub fn archivable(&self) -> Vec<CardKey> {
        self.column(Status::Done)
            .into_iter()
            .filter(|c| c.workspace != "archive")
            .map(|c| c.key.clone())
            .collect()
    }

    /// Move every done card on show into its project's `archive/` folder. The file content
    /// does not change, only its path. Returns how many moved.
    pub fn archive_done(&mut self) -> Result<usize, String> {
        let mut ops = Vec::new();
        let mut failure = None;
        for from in self.archivable() {
            let moved = (|| {
                let store = self.store_for(&from)?;
                let file_name = from.rel_path.rsplit('/').next().unwrap_or(&from.rel_path);
                let rel_path = store
                    .free_name("archive", file_name)
                    .map_err(|e| e.to_string())?;
                store
                    .rename(&from.rel_path, &rel_path)
                    .map_err(|e| e.to_string())?;
                let text = store.read(&rel_path).map_err(|e| e.to_string())?;
                Ok::<_, String>((
                    CardKey {
                        project: from.project.clone(),
                        rel_path,
                    },
                    text,
                ))
            })();
            match moved {
                Ok((to, text)) => {
                    self.reload(&from, None);
                    self.reload(&to, Some(&text));
                    ops.push(FileOp::Rename { from, to });
                }
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let count = ops.len();
        self.record(format!("Archive {count} cards"), ops);
        match failure {
            Some(error) if count == 0 => Err(error),
            _ => Ok(count),
        }
    }

    /// Flip the nth checkbox in a card's body.
    pub fn toggle_subtask(&mut self, key: &CardKey, index: usize) -> Result<(), String> {
        let body = &self.card(key).ok_or("That card is gone")?.data.body;
        let patch = CardPatch {
            body: Some(mdk_format::toggle_subtask(body, index)),
            ..Default::default()
        };
        self.update_card(key, &patch)
    }

    // ---- undo and redo ----------------------------------------------------------------------

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Undo the last board action. Returns its label. Refused, and left on the stack, when
    /// a file it touched has changed on disk since.
    pub fn undo(&mut self) -> Result<Option<String>, String> {
        let Some(entry) = self.undo.pop() else {
            return Ok(None);
        };
        match self.replay(&entry, true) {
            Ok(()) => {
                let label = entry.label.clone();
                self.redo.push(entry);
                Ok(Some(label))
            }
            Err(error) => {
                self.undo.push(entry);
                Err(error)
            }
        }
    }

    pub fn redo(&mut self) -> Result<Option<String>, String> {
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        match self.replay(&entry, false) {
            Ok(()) => {
                let label = entry.label.clone();
                self.undo.push(entry);
                Ok(Some(label))
            }
            Err(error) => {
                self.redo.push(entry);
                Err(error)
            }
        }
    }

    /// Run an entry's file changes backwards (undo) or forwards (redo). Every file is checked
    /// first, so the entry is applied whole or not at all.
    fn replay(&mut self, entry: &UndoEntry, backwards: bool) -> Result<(), String> {
        const CHANGED: &str = "Changed on disk since, can't undo";
        // (key, the text the file must have now, the text it gets), or a rename.
        let mut steps = Vec::new();
        for op in &entry.ops {
            steps.push(match (op, backwards) {
                (FileOp::Change { key, before, after }, true) => {
                    (key.clone(), after.clone(), before.clone(), None)
                }
                (FileOp::Change { key, before, after }, false) => {
                    (key.clone(), before.clone(), after.clone(), None)
                }
                (FileOp::Rename { from, to }, true) => (to.clone(), None, None, Some(from.clone())),
                (FileOp::Rename { from, to }, false) => {
                    (from.clone(), None, None, Some(to.clone()))
                }
            });
        }
        if backwards {
            steps.reverse();
        }
        for (key, expected, _, rename_to) in &steps {
            let store = self.store_for(key)?;
            let current = store.read(&key.rel_path).ok();
            let ok = match rename_to {
                Some(to) => current.is_some() && store.read(&to.rel_path).is_err(),
                None => current == *expected,
            };
            if !ok {
                return Err(CHANGED.to_string());
            }
        }
        for (key, _, target, rename_to) in steps {
            let store = self.store_for(&key)?;
            let done = match (&rename_to, &target) {
                (Some(to), _) => store.rename(&key.rel_path, &to.rel_path),
                (None, Some(text)) => store.write(&key.rel_path, text),
                (None, None) => store.delete(&key.rel_path).map(drop),
            };
            done.map_err(|e| e.to_string())?;
            match rename_to {
                Some(to) => {
                    let text = store.read(&to.rel_path).ok();
                    self.reload(&key, None);
                    self.reload(&to, text.as_deref());
                }
                None => self.reload(&key, target.as_deref()),
            }
        }
        Ok(())
    }

    // ---- selection --------------------------------------------------------------------------

    /// Select a project, or `None` for all. The work folder goes back to "all".
    pub fn select_project(&mut self, id: Option<ProjectId>) {
        self.selected_project = id.filter(|id| self.project(id).is_some());
        self.selected_workspace = None;
    }

    pub fn select_workspace(&mut self, workspace: Option<String>) {
        self.selected_workspace = workspace;
    }

    /// Whether the selected project and work folder show this card. `archive/` is hidden
    /// unless it is the work folder selected.
    fn in_selection(&self, card: &Card) -> bool {
        if self
            .selected_project
            .as_ref()
            .is_some_and(|id| *id != card.key.project)
        {
            return false;
        }
        match &self.selected_workspace {
            None => card.workspace != "archive",
            Some(workspace) => card.workspace == *workspace,
        }
    }

    // ---- queries ----------------------------------------------------------------------------

    pub fn card(&self, key: &CardKey) -> Option<&Card> {
        self.cards.get(key)
    }

    pub fn card_count(&self) -> usize {
        self.cards.len()
    }

    /// The cards the selection shows, sorted by `order`. Equal orders sort by path, so the
    /// board does not shuffle between refreshes.
    pub fn visible_cards(&self) -> Vec<&Card> {
        let mut cards: Vec<&Card> = self
            .cards
            .values()
            .filter(|c| self.in_selection(c))
            .collect();
        cards.sort_by(|a, b| {
            a.data
                .order
                .total_cmp(&b.data.order)
                .then_with(|| a.key.cmp(&b.key))
        });
        cards
    }

    pub fn column(&self, status: Status) -> Vec<&Card> {
        let mut cards = self.visible_cards();
        cards.retain(|c| c.data.status == status);
        cards
    }

    /// The work folders of the selected project, or of every project, sorted.
    pub fn workspaces(&self) -> Vec<String> {
        let names: BTreeSet<&str> = self
            .cards
            .values()
            .filter(|c| {
                self.selected_project
                    .as_ref()
                    .is_none_or(|id| *id == c.key.project)
            })
            .map(|c| c.workspace.as_str())
            .filter(|w| !w.is_empty())
            .collect();
        names.into_iter().map(str::to_string).collect()
    }

    /// Cards that are not done, per project.
    pub fn open_counts(&self) -> HashMap<&str, usize> {
        let mut counts = HashMap::new();
        for card in self
            .cards
            .values()
            .filter(|c| c.data.status != Status::Done)
        {
            *counts.entry(card.key.project.as_str()).or_insert(0) += 1;
        }
        counts
    }

    /// True while a first scan is still running and there is nothing to show yet.
    pub fn is_loading(&self) -> bool {
        self.cards.is_empty() && self.projects.iter().any(|p| p.loading)
    }
}

#[cfg(test)]
mod tests {
    use super::{Board, CardKey, ProjectStatus};
    use mdk_format::Status;
    use mdk_store::StoreEvent;
    use std::fs;
    use std::path::Path;

    fn card(status: &str, order: i32) -> String {
        format!("---\nstatus: \"{status}\"\norder: {order}\n---\n# Card\n")
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A board with one scanned project holding the given files.
    fn board(files: &[(&str, String)]) -> (tempfile::TempDir, Board, String) {
        let dir = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            put(dir.path(), rel, text);
        }
        let mut board = Board::default();
        let id = board.connect_project(dir.path()).unwrap();
        board.scan_now(&id);
        (dir, board, id)
    }

    fn paths(board: &Board) -> Vec<&str> {
        board
            .visible_cards()
            .iter()
            .map(|c| c.key.rel_path.as_str())
            .collect()
    }

    #[test]
    fn connecting_selects_the_project_and_names_it_after_the_folder() {
        let (dir, mut board, id) = board(&[("backend/x.md", card("todo", 1))]);
        let project = board.project(&id).unwrap();
        assert_eq!(
            project.name,
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!((id.len(), &project.status), (8, &ProjectStatus::Ok));
        assert_eq!(board.selected_project.as_deref(), Some(id.as_str()));
        // Connecting the same folder again adds nothing.
        assert_eq!(board.connect_project(dir.path()).unwrap(), id);
        assert_eq!(board.projects.len(), 1);
        assert!(board.connect_project(&dir.path().join("nope")).is_err());
    }

    #[test]
    fn cards_sort_by_order_and_columns_split_by_status() {
        let (_dir, board, _) = board(&[
            ("c.md", card("todo", 30)),
            ("a.md", card("todo", 10)),
            ("b.md", card("done", 20)),
            ("d.md", card("todo", 10)),
        ]);
        assert_eq!(paths(&board), ["a.md", "d.md", "b.md", "c.md"]);
        let todo: Vec<_> = board
            .column(Status::Todo)
            .iter()
            .map(|c| c.key.rel_path.clone())
            .collect();
        assert_eq!(todo, ["a.md", "d.md", "c.md"]);
        assert!(board.column(Status::Review).is_empty());
    }

    #[test]
    fn work_folders_filter_and_archive_is_hidden_until_selected() {
        let (_dir, mut board, id) = board(&[
            ("root.md", card("todo", 1)),
            ("backend/x.md", card("todo", 2)),
            ("board/sub/y.md", card("todo", 3)),
            ("archive/old.md", card("done", 4)),
        ]);
        assert_eq!(board.workspaces(), ["archive", "backend", "board"]);
        assert_eq!(paths(&board), ["root.md", "backend/x.md", "board/sub/y.md"]);
        board.select_workspace(Some("board".into()));
        assert_eq!(paths(&board), ["board/sub/y.md"]);
        board.select_workspace(Some("archive".into()));
        assert_eq!(paths(&board), ["archive/old.md"]);
        // Picking a project puts the work folder back to "all".
        board.select_project(Some(id));
        assert_eq!(board.selected_workspace, None);
    }

    #[test]
    fn open_counts_leave_out_done_cards() {
        let mut files: Vec<(String, String)> = (0..27)
            .map(|i| (format!("t{i}.md"), card("todo", i)))
            .collect();
        files.extend((0..10).map(|i| (format!("d{i}.md"), card("done", i))));
        let files: Vec<(&str, String)> =
            files.iter().map(|(p, t)| (p.as_str(), t.clone())).collect();
        let (_dir, board, id) = board(&files);
        assert_eq!(board.card_count(), 37);
        assert_eq!(board.open_counts()[id.as_str()], 27);
    }

    #[test]
    fn two_projects_show_together_and_filter_apart() {
        let (_a, mut board, a) = board(&[("x.md", card("todo", 1))]);
        let other = tempfile::tempdir().unwrap();
        put(other.path(), "ui/y.md", &card("review", 2));
        let b = board.connect_project(other.path()).unwrap();
        board.scan_now(&b);

        assert_eq!(paths(&board), ["ui/y.md"]);
        board.select_project(None);
        assert_eq!(paths(&board), ["x.md", "ui/y.md"]);
        assert_eq!(board.workspaces(), ["ui"]);
        board.select_project(Some(a.clone()));
        assert!(board.workspaces().is_empty());

        // Removing a project forgets its cards and never touches its files.
        board.remove_project(&a);
        assert_eq!(
            (board.projects.len(), board.selected_project.clone()),
            (1, None)
        );
        assert_eq!(paths(&board), ["ui/y.md"]);
        assert!(_a.path().join("x.md").exists());
    }

    #[test]
    fn watcher_events_update_single_cards() {
        let (dir, mut board, id) = board(&[("a.md", card("todo", 1)), ("b.md", card("todo", 2))]);
        let key = |rel: &str| CardKey {
            project: id.clone(),
            rel_path: rel.into(),
        };

        put(dir.path(), "a.md", &card("done", 1));
        put(dir.path(), "new/c.md", &card("review", 3));
        fs::remove_file(dir.path().join("b.md")).unwrap();
        let rescan = board.apply_events(
            &id,
            vec![
                StoreEvent::Changed {
                    rel_path: "a.md".into(),
                },
                StoreEvent::Changed {
                    rel_path: "new/c.md".into(),
                },
                StoreEvent::Removed {
                    rel_path: "b.md".into(),
                },
            ],
        );
        assert!(!rescan);
        assert_eq!(board.card(&key("a.md")).unwrap().data.status, Status::Done);
        assert_eq!(board.card(&key("new/c.md")).unwrap().workspace, "new");
        assert!(board.card(&key("b.md")).is_none());
        assert!(board.apply_events(&id, vec![StoreEvent::Rescan]));
    }

    #[test]
    fn a_missing_folder_keeps_its_cards_and_can_be_relocated() {
        let (dir, mut board, id) = board(&[("a.md", card("todo", 1))]);
        let moved = dir.path().with_extension("moved");
        fs::rename(dir.path(), &moved).unwrap();

        board.scan_now(&id);
        assert_eq!(board.project(&id).unwrap().status, ProjectStatus::Missing);
        assert_eq!(
            paths(&board),
            ["a.md"],
            "the last cards read stay on screen"
        );

        put(&moved, "b.md", &card("todo", 2));
        board.relocate_project(&id, &moved).unwrap();
        board.scan_now(&id);
        assert_eq!(board.project(&id).unwrap().status, ProjectStatus::Ok);
        assert_eq!(paths(&board), ["a.md", "b.md"]);
        fs::rename(moved, dir.path()).unwrap();
    }

    #[test]
    fn broken_and_unreadable_files_are_still_cards() {
        let (dir, mut board, id) =
            board(&[("broken.md", "---\nstatus: [todo\n---\n# Broken\n".into())]);
        fs::write(dir.path().join("binary.md"), [0xff, 0xfe, 0x00]).unwrap();
        board.scan_now(&id);
        let cards = board.visible_cards();
        assert_eq!(cards.len(), 2);
        let by = |rel: &str| cards.iter().find(|c| c.key.rel_path == rel).unwrap();
        assert!(by("broken.md").error().is_some());
        assert_eq!(by("broken.md").data.title, "Broken");
        assert_eq!(by("binary.md").error(), Some("not UTF-8 text"));
        assert_eq!(by("binary.md").data.title, "binary");
    }

    #[test]
    fn card_face_fields_are_derived() {
        let text = "---\nid: \"x\"\nassignee: \"Safi\"\nlabels: [\"Auth\"]\n---\n# Fix Login\n\nNotes %% private %% here.\n- [x] one\n- [ ] two\n";
        let (_dir, board, _) = board(&[("backend/x.md", text.into())]);
        let card = board.visible_cards()[0];
        assert_eq!(card.workspace, "backend");
        assert_eq!(card.subtasks, (1, 2));
        assert!(card.excerpt.starts_with("Notes  here."));
        assert!(card.search_text.contains("fix login") && card.search_text.contains("safi auth"));
    }
}
