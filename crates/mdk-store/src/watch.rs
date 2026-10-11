//! Live updates: a debounced `notify` watcher that reports what other programs changed.

use crate::{Result, Store, StoreError, content_hash, is_card_path};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(150);
/// A batch touching more paths than this becomes one `Rescan` (a big checkout, say).
const BURST: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreEvent {
    /// A card file was created or its content changed.
    Changed {
        rel_path: String,
    },
    /// A card file is gone. A rename arrives as `Removed` plus `Changed`.
    Removed {
        rel_path: String,
    },
    /// Too much changed to list, or a folder moved. Read the project again.
    Rescan,
    /// The project folder itself is gone. Sent with the next event after it went. The OS
    /// does not always send one, so callers should also check the folder from time to time.
    RootMissing,
    Error {
        message: String,
    },
}

/// Keeps the watch alive. Dropping it stops the watch.
pub struct Watcher {
    _debouncer: Box<dyn std::any::Any + Send>,
}

impl Watcher {
    pub(crate) fn start(
        store: Store,
        on_batch: impl Fn(Vec<StoreEvent>) + Send + 'static,
    ) -> Result<Watcher> {
        let root = store.root().to_path_buf();
        let watched = store.clone();
        let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
            let batch = match result {
                Ok(events) => {
                    let paths = events
                        .iter()
                        .filter(|e| !e.kind.is_access())
                        .flat_map(|e| e.paths.iter().map(|p| p.as_path()));
                    watched.batch(paths)
                }
                Err(errors) => errors
                    .iter()
                    .map(|e| StoreEvent::Error {
                        message: e.to_string(),
                    })
                    .collect(),
            };
            if !batch.is_empty() {
                on_batch(batch);
            }
        })
        .map_err(watch_error)?;
        debouncer
            .watch(&root, RecursiveMode::Recursive)
            .map_err(watch_error)?;
        Ok(Watcher {
            _debouncer: Box::new(debouncer),
        })
    }
}

fn watch_error(e: notify::Error) -> StoreError {
    StoreError::Rejected(format!("Can't watch the folder: {e}"))
}

impl Store {
    /// Turn the paths of one debounced batch into events, leaving out the store's own writes.
    ///
    /// Only the paths are used, never the event kinds. On macOS the kinds are a mix of
    /// everything that happened to a path lately, and the old name of a renamed file is
    /// not reported at all. So each path is checked on disk, and a path the store has not
    /// seen before triggers a sweep for known cards that have vanished.
    fn batch<'a>(&self, paths: impl Iterator<Item = &'a Path>) -> Vec<StoreEvent> {
        if !self.root().is_dir() {
            return vec![StoreEvent::RootMissing];
        }
        let mut cards = BTreeSet::new();
        let mut rescan = false;
        for path in paths {
            let Some(rel_path) = path
                .strip_prefix(self.root())
                .ok()
                .and_then(|rel| rel.to_str())
                .map(|rel| rel.replace('\\', "/"))
            else {
                continue;
            };
            if is_card_path(&rel_path) {
                cards.insert(rel_path);
            } else if !rel_path.is_empty()
                && !rel_path.split('/').any(|part| part.starts_with('.'))
                && (path.is_dir() || !path.exists())
            {
                // A folder appeared, moved or went. Its cards may have come or gone with it.
                rescan |= Path::new(&rel_path).extension().is_none() || path.is_dir();
            }
        }
        if rescan || cards.len() > BURST {
            return vec![StoreEvent::Rescan];
        }

        let exists = |rel_path: &str| self.resolve(rel_path).is_ok_and(|p| p.is_file());
        let mut known = self.known.lock().unwrap();
        let mut new_path = false;
        for rel_path in &cards {
            if exists(rel_path) {
                new_path |= known.insert(rel_path.clone());
            }
        }
        if new_path {
            cards.extend(known.iter().filter(|k| !exists(k)).cloned());
        }
        cards
            .into_iter()
            .filter_map(|rel_path| {
                let here = exists(&rel_path);
                if !here {
                    known.remove(&rel_path);
                }
                if self.is_own(&rel_path, self.current(&rel_path)) {
                    None
                } else if here {
                    Some(StoreEvent::Changed { rel_path })
                } else {
                    Some(StoreEvent::Removed { rel_path })
                }
            })
            .collect()
    }

    /// The hash of the file as it is now, or `None` if it cannot be read.
    fn current(&self, rel_path: &str) -> Option<u64> {
        self.read(rel_path).ok().map(|text| content_hash(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn more_than_200_changed_cards_in_a_batch_become_one_rescan() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let paths: Vec<_> = (0..=BURST)
            .map(|i| store.root().join(format!("c{i}.md")))
            .collect();
        for path in &paths {
            fs::write(path, "x").unwrap();
        }
        let batch = |n: usize| store.batch(paths[..n].iter().map(|p| p.as_path()));
        assert_eq!(batch(BURST).len(), BURST);
        assert_eq!(batch(BURST + 1), [StoreEvent::Rescan]);
    }

    #[test]
    fn any_event_after_the_root_is_gone_reports_it_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let card = store.root().join("card.md");
        fs::write(&card, "x").unwrap();
        assert_eq!(store.batch([card.as_path()].into_iter()).len(), 1);
        fs::remove_dir_all(dir.path()).unwrap();
        assert_eq!(
            store.batch([card.as_path()].into_iter()),
            [StoreEvent::RootMissing]
        );
    }
}
