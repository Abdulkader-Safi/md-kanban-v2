//! File access for one project: a folder of markdown cards.
//!
//! Everything goes through a [`Store`], which is bound to one root folder and refuses
//! any path that leaves it. Writes are atomic. The store remembers the hash of what it
//! wrote, so the watcher can tell its own writes from an agent's.
//!
//! The store knows nothing about the card format. It deals in paths and text.

mod watch;

pub use watch::{StoreEvent, Watcher};

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SKIP_DIRS: [&str; 4] = ["node_modules", "dist", "out", ".git"];
/// A card file can sit at most this many folders below the root.
const MAX_DEPTH: usize = 4;
/// Bigger card files are listed but not read (NFR-04).
pub const MAX_CARD_BYTES: u64 = 1024 * 1024;
const IMAGE_TYPES: [&str; 9] = [
    "png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "bmp", "ico",
];

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// The path leaves the project folder, by `..`, an absolute path or a symlink.
    OutsideRoot(String),
    NotFound(String),
    /// Not an image file type.
    NotAnImage(String),
    /// The caller's change could not be applied to the file's current content.
    Rejected(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::OutsideRoot(p) => write!(f, "{p} is outside the project folder"),
            StoreError::NotFound(p) => write!(f, "{p} was not found"),
            StoreError::NotAnImage(p) => write!(f, "{p} is not an image"),
            StoreError::Rejected(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// One `.md` file found by a scan. `content` is an error message when the file is
/// too large or is not UTF-8.
#[derive(Debug, PartialEq)]
pub struct ScannedFile {
    /// Relative to the root, with `/` separators on every OS.
    pub rel_path: String,
    pub content: std::result::Result<String, String>,
}

pub fn content_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Whether `rel_path` is a card under the scan rules: a `.md` file, at most four folders
/// deep, with no hidden part and no skipped folder on the way.
pub fn is_card_path(rel_path: &str) -> bool {
    let parts: Vec<&str> = rel_path.split('/').collect();
    let Some((file, dirs)) = parts.split_last() else {
        return false;
    };
    dirs.len() <= MAX_DEPTH
        && file.to_lowercase().ends_with(".md")
        && parts.iter().all(|p| !p.is_empty() && !p.starts_with('.'))
        && dirs.iter().all(|d| !SKIP_DIRS.contains(d))
}

/// Write through a temp file in the same folder, then rename over the target.
/// `commit` does the rename. Tests pass one that fails, to stand in for a crash.
fn atomic_write_with(
    path: &Path,
    bytes: &[u8],
    commit: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    // Hidden, so a scan or another app's watcher skips it for the moment it exists.
    let tmp = path.with_file_name(format!(
        ".{}.mdk-tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        commit(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Write `bytes` to `path` so that a crash leaves either the old file or the new one.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // std::fs::rename replaces the target on every OS (MoveFileExW with replace on Windows).
    atomic_write_with(path, bytes, |tmp, path| fs::rename(tmp, path))
}

/// How long a superseded write still counts as the store's own. The watcher can look at a
/// file between two quick saves and find the first one's content there.
const OWN_WRITE_GRACE: Duration = Duration::from_secs(1);

/// What the store did to each path, oldest first: the hash it wrote (`None` for a delete)
/// and when. The last entry is the current one. Older ones only bridge two quick saves.
type OwnWrites = Arc<Mutex<HashMap<String, Vec<(Option<u64>, Instant)>>>>;

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    own: OwnWrites,
    /// The card paths seen so far, from the last scan plus later changes.
    known: Arc<Mutex<BTreeSet<String>>>,
}

impl Store {
    /// Bind a store to a folder. Fails if the folder is missing or is not a folder.
    pub fn open(root: &Path) -> Result<Store> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(StoreError::NotFound(root.display().to_string()));
        }
        Ok(Store {
            root,
            own: Arc::default(),
            known: Arc::default(),
        })
    }

    /// The canonical root folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path guard. Turns a relative path into an absolute one inside the root.
    pub fn resolve(&self, rel_path: &str) -> Result<PathBuf> {
        let outside = || StoreError::OutsideRoot(rel_path.to_string());
        let rel = Path::new(rel_path);
        if rel_path.is_empty() || !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(outside());
        }
        let path = self.root.join(rel);
        // Symlinks: resolve the deepest part that exists, and check where it really is.
        let mut existing = path.as_path();
        while !existing.exists() {
            existing = existing.parent().ok_or_else(outside)?;
        }
        if !existing.canonicalize()?.starts_with(&self.root) {
            return Err(outside());
        }
        Ok(path)
    }

    /// Every card file under the root, by Electron's rules. Symlinks are not followed.
    pub fn scan(&self) -> Result<Vec<ScannedFile>> {
        // Fail when the root itself is gone. Unreadable subfolders are skipped.
        fs::read_dir(&self.root)?;
        let mut out = Vec::new();
        self.walk(&self.root, "", 0, &mut out);
        out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        *self.known.lock().unwrap() = out.iter().map(|f| f.rel_path.clone()).collect();
        Ok(out)
    }

    fn walk(&self, dir: &Path, rel: &str, depth: usize, out: &mut Vec<ScannedFile>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let rel_path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if kind.is_dir() {
                if depth < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) {
                    self.walk(&entry.path(), &rel_path, depth + 1, out);
                }
            } else if kind.is_file() && name.to_lowercase().ends_with(".md") {
                let content = read_card(&entry.path());
                out.push(ScannedFile { rel_path, content });
            }
        }
    }

    /// Read one card file. Missing files are `NotFound`.
    pub fn read(&self, rel_path: &str) -> Result<String> {
        let path = self.resolve(rel_path)?;
        if !path.is_file() {
            return Err(StoreError::NotFound(rel_path.to_string()));
        }
        read_card(&path).map_err(StoreError::Rejected)
    }

    /// Write a card file atomically, creating its folder if needed.
    pub fn write(&self, rel_path: &str, text: &str) -> Result<()> {
        let path = self.resolve(rel_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Record first, so the watcher already knows this content when the event arrives.
        self.record(rel_path, Some(content_hash(text)));
        atomic_write(&path, text.as_bytes()).inspect_err(|_| self.forget(rel_path))?;
        Ok(())
    }

    /// Re-read the file, let `change` produce the new text from what is on disk now, and
    /// write it. Returns the text before and after. This is how every edit is made, so a
    /// change another program made a moment ago is never overwritten by stale state.
    pub fn update(
        &self,
        rel_path: &str,
        change: impl FnOnce(&str) -> std::result::Result<String, String>,
    ) -> Result<(String, String)> {
        let before = self.read(rel_path)?;
        let after = change(&before).map_err(StoreError::Rejected)?;
        self.write(rel_path, &after)?;
        Ok((before, after))
    }

    /// Delete a card file and return its text, so the delete can be undone.
    pub fn delete(&self, rel_path: &str) -> Result<String> {
        let text = self.read(rel_path)?;
        self.record(rel_path, None);
        fs::remove_file(self.resolve(rel_path)?).inspect_err(|_| self.forget(rel_path))?;
        Ok(text)
    }

    /// Move a card file. Refuses to replace an existing file.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let text = self.read(from)?;
        let (source, target) = (self.resolve(from)?, self.resolve(to)?);
        if target.exists() {
            return Err(StoreError::Rejected(format!("{to} already exists")));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        self.record(from, None);
        self.record(to, Some(content_hash(&text)));
        fs::rename(source, target).inspect_err(|_| {
            self.forget(from);
            self.forget(to);
        })?;
        Ok(())
    }

    /// A name in `dir` (relative, or empty for the root) that no file has yet:
    /// `name.md`, then `name-2.md`, `name-3.md` and so on.
    pub fn free_name(&self, dir: &str, file_name: &str) -> Result<String> {
        let (stem, ext) = file_name.rsplit_once('.').unwrap_or((file_name, "md"));
        let join = |name: String| {
            if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            }
        };
        let mut candidate = join(file_name.to_string());
        let mut n = 2;
        while self.resolve(&candidate)?.exists() {
            candidate = join(format!("{stem}-{n}.{ext}"));
            n += 1;
        }
        Ok(candidate)
    }

    /// Image bytes for the preview. Only image file types, only inside the root.
    pub fn read_image(&self, rel_path: &str) -> Result<Vec<u8>> {
        let path = self.resolve(rel_path)?;
        let is_image = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| IMAGE_TYPES.contains(&e.to_lowercase().as_str()));
        if !is_image {
            return Err(StoreError::NotAnImage(rel_path.to_string()));
        }
        if !path.is_file() {
            return Err(StoreError::NotFound(rel_path.to_string()));
        }
        Ok(fs::read(path)?)
    }

    /// Watch the root for changes made by other programs. Events arrive in batches on a
    /// background thread. Drop the returned watcher to stop. Call [`Store::scan`] first:
    /// the watcher finds renamed-away files by checking the paths the scan saw.
    pub fn watch(&self, on_batch: impl Fn(Vec<StoreEvent>) + Send + 'static) -> Result<Watcher> {
        Watcher::start(self.clone(), on_batch)
    }

    /// Note what the store is about to do to a path.
    fn record(&self, rel_path: &str, hash: Option<u64>) {
        let mut known = self.known.lock().unwrap();
        match hash {
            Some(_) => known.insert(rel_path.to_string()),
            None => known.remove(rel_path),
        };
        let mut own = self.own.lock().unwrap();
        let entries = own.entry(rel_path.to_string()).or_default();
        // Keep only the write this one supersedes. Anything older is long settled.
        let keep_from = entries.len().saturating_sub(1);
        entries.drain(..keep_from);
        entries.push((hash, Instant::now()));
    }

    /// The write just recorded did not happen. Drop it.
    fn forget(&self, rel_path: &str) {
        if let Some(entries) = self.own.lock().unwrap().get_mut(rel_path) {
            entries.pop();
        }
    }

    /// Whether the file's current state is something the store itself wrote there.
    /// `current` is the hash of the file now, or `None` if it does not exist.
    fn is_own(&self, rel_path: &str, current: Option<u64>) -> bool {
        let own = self.own.lock().unwrap();
        let Some(entries) = own.get(rel_path) else {
            return false;
        };
        entries.iter().enumerate().any(|(i, (hash, _))| {
            *hash == current
                && match entries.get(i + 1) {
                    // The current write always counts.
                    None => true,
                    // A superseded one counts only while the newer write is still landing.
                    Some((_, superseded_at)) => superseded_at.elapsed() < OWN_WRITE_GRACE,
                }
        })
    }
}

fn read_card(path: &Path) -> std::result::Result<String, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_CARD_BYTES {
        return Err(format!("too large ({} KB)", size / 1024));
    }
    String::from_utf8(fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|_| "not UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_superseded_write_counts_as_own_only_briefly() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let (v1, v2, other) = (
            Some(content_hash("v1")),
            Some(content_hash("v2")),
            Some(content_hash("agent")),
        );
        store.write("card.md", "v1").unwrap();
        store.write("card.md", "v2").unwrap();
        // The watcher may still find v1 on disk for a moment. Both are the store's own.
        assert!(store.is_own("card.md", v2) && store.is_own("card.md", v1));
        assert!(!store.is_own("card.md", other) && !store.is_own("card.md", None));
        // Later, v1 on disk means another program put it back, and that must be reported.
        std::thread::sleep(OWN_WRITE_GRACE + Duration::from_millis(50));
        assert!(store.is_own("card.md", v2) && !store.is_own("card.md", v1));
        assert!(!store.is_own("other.md", v1));
    }

    #[test]
    fn a_failed_write_leaves_the_original_and_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("card.md");
        fs::write(&card, "original").unwrap();
        // The new content is fully written to the temp file, then the "crash" comes.
        let crash = atomic_write_with(&card, b"half-finished new content", |tmp, _| {
            assert_eq!(
                fs::read_to_string(tmp).unwrap(),
                "half-finished new content"
            );
            Err(io::Error::other("power cut"))
        });
        assert!(crash.is_err());
        assert_eq!(fs::read_to_string(&card).unwrap(), "original");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "temp file left behind"
        );
    }
}
