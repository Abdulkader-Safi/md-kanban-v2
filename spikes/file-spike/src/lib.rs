//! M0 file spike: a debounced `notify` watcher plus atomic writes.
//! Throwaway. The production version lands in `mdk-store` in M1.

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_millis(150);

/// Write `bytes` to `path` through a temp file in the same folder, then rename over it.
/// The temp name starts with a dot so a scan that skips hidden entries never sees it.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let tmp = path.with_file_name(format!(
        ".{}.mdk-tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        // std::fs::rename replaces the target on every OS (MoveFileExW with replace on Windows).
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// One debounced batch: the set of `.md` paths touched.
pub type Batch = BTreeSet<PathBuf>;

/// Watch `root` recursively. Keep the returned guard alive for as long as you watch.
pub fn watch(root: &Path) -> notify::Result<(impl Sized + use<>, Receiver<Batch>)> {
    let (tx, rx) = channel();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let batch: Batch = events
                .iter()
                .filter(|e| !e.kind.is_access())
                .flat_map(|e| e.paths.iter().cloned())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect();
            if !batch.is_empty() {
                let _ = tx.send(batch);
            }
        }
    })?;
    debouncer.watch(root, RecursiveMode::Recursive)?;
    Ok((debouncer, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // Shared CI machines stall now and then. A real machine has to make 500 ms.
    const LIMIT: Duration = Duration::from_millis(if option_env!("CI").is_some() {
        3000
    } else {
        500
    });

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        // macOS temp dirs sit behind a /var -> /private/var symlink, and FSEvents reports real paths.
        let root = dir.path().canonicalize().unwrap();
        (dir, root)
    }

    /// Let the watcher start and drain whatever setup produced.
    fn settle(rx: &Receiver<Batch>) {
        std::thread::sleep(Duration::from_millis(400));
        while rx.try_recv().is_ok() {}
    }

    /// Wait for a batch containing `path`. Returns the latency.
    fn wait_for(rx: &Receiver<Batch>, path: &Path, start: Instant) -> Duration {
        loop {
            let batch = rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("no event for {}", path.display()));
            if batch.contains(path) {
                return start.elapsed();
            }
        }
    }

    #[test]
    fn external_edit_is_seen_within_500ms() {
        let (_dir, root) = setup();
        let card = root.join("card.md");
        fs::write(&card, "---\nstatus: \"todo\"\n---\n# A\n").unwrap();
        let (_guard, rx) = watch(&root).unwrap();
        settle(&rx);

        let start = Instant::now();
        fs::write(&card, "---\nstatus: \"done\"\n---\n# A\n").unwrap();
        let took = wait_for(&rx, &card, start);
        assert!(took < LIMIT, "edit took {took:?}");
    }

    #[test]
    fn rename_save_is_seen_as_a_change_to_the_card() {
        let (_dir, root) = setup();
        let card = root.join("card.md");
        fs::write(&card, "old").unwrap();
        let (_guard, rx) = watch(&root).unwrap();
        settle(&rx);

        // What vim and git do: write a sibling, then rename it over the card.
        let start = Instant::now();
        let swap = root.join("card.md.swp~");
        fs::write(&swap, "new").unwrap();
        fs::rename(&swap, &card).unwrap();
        let took = wait_for(&rx, &card, start);
        assert!(took < LIMIT, "rename-save took {took:?}");
        assert_eq!(fs::read_to_string(&card).unwrap(), "new");
    }

    #[test]
    fn our_own_atomic_write_is_seen_and_leaves_no_temp_file() {
        let (_dir, root) = setup();
        let card = root.join("card.md");
        fs::write(&card, "old").unwrap();
        let (_guard, rx) = watch(&root).unwrap();
        settle(&rx);

        let start = Instant::now();
        atomic_write(&card, b"new").unwrap();
        let took = wait_for(&rx, &card, start);
        assert!(took < LIMIT, "atomic write took {took:?}");
        assert_eq!(fs::read_to_string(&card).unwrap(), "new");
        let names: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["card.md"], "temp file left behind");
    }

    #[test]
    fn atomic_write_replaces_a_file_another_handle_has_open() {
        let (_dir, root) = setup();
        let card = root.join("card.md");
        fs::write(&card, "old").unwrap();
        // Rust opens with FILE_SHARE_DELETE on Windows, like most editors. This is the case that must work.
        let reader = File::open(&card).unwrap();
        atomic_write(&card, b"new").unwrap();
        drop(reader);
        assert_eq!(fs::read_to_string(&card).unwrap(), "new");
    }

    #[test]
    fn burst_of_50_files_settles_into_few_batches() {
        let (_dir, root) = setup();
        let cards: Vec<_> = (0..50).map(|i| root.join(format!("c{i}.md"))).collect();
        for c in &cards {
            fs::write(c, "old").unwrap();
        }
        let (_guard, rx) = watch(&root).unwrap();
        settle(&rx);

        let start = Instant::now();
        for c in &cards {
            fs::write(c, "new").unwrap();
        }
        let mut seen = Batch::new();
        let mut batches = 0;
        while seen.len() < cards.len() {
            let batch = rx.recv_timeout(Duration::from_secs(5)).expect("burst lost");
            seen.extend(batch);
            batches += 1;
        }
        let took = start.elapsed();
        assert!(took < LIMIT, "burst took {took:?}");
        // The Board coalesces whatever is queued into one refresh, so a handful is fine.
        assert!(batches <= 3, "burst arrived in {batches} batches");
    }

    #[test]
    fn new_subfolder_is_watched() {
        let (_dir, root) = setup();
        let (_guard, rx) = watch(&root).unwrap();
        settle(&rx);

        let sub = root.join("backend");
        fs::create_dir(&sub).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let card = sub.join("card.md");
        let start = Instant::now();
        fs::write(&card, "x").unwrap();
        let took = wait_for(&rx, &card, start);
        assert!(took < LIMIT, "new-folder card took {took:?}");
    }
}
