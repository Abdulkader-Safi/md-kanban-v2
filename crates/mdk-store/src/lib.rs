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

/// What the store last did to a path: the hash it wrote, or `None` for a delete.
type OwnWrites = Arc<Mutex<HashMap<String, Option<u64>>>>;

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
        let previous = self.record(rel_path, Some(content_hash(text)));
        atomic_write(&path, text.as_bytes()).inspect_err(|_| self.restore(rel_path, previous))?;
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
        let previous = self.record(rel_path, None);
        fs::remove_file(self.resolve(rel_path)?)
            .inspect_err(|_| self.restore(rel_path, previous))?;
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
        let previous = (
            self.record(from, None),
            self.record(to, Some(content_hash(&text))),
        );
        fs::rename(source, target).inspect_err(|_| {
            self.restore(from, previous.0);
            self.restore(to, previous.1);
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

    fn record(&self, rel_path: &str, hash: Option<u64>) -> Option<Option<u64>> {
        let mut known = self.known.lock().unwrap();
        match hash {
            Some(_) => known.insert(rel_path.to_string()),
            None => known.remove(rel_path),
        };
        self.own.lock().unwrap().insert(rel_path.to_string(), hash)
    }

    fn restore(&self, rel_path: &str, previous: Option<Option<u64>>) {
        let mut own = self.own.lock().unwrap();
        match previous {
            Some(hash) => own.insert(rel_path.to_string(), hash),
            None => own.remove(rel_path),
        };
    }

    /// Whether the file's current state is what the store itself last wrote there.
    /// `current` is the hash of the file now, or `None` if it does not exist.
    fn is_own(&self, rel_path: &str, current: Option<u64>) -> bool {
        self.own.lock().unwrap().get(rel_path) == Some(&current)
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
