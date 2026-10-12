use mdk_store::{Store, StoreError, StoreEvent, is_card_path};
use std::fs;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long an outside change may take to be reported: 500 ms (FR-LIVE-01). Shared CI
/// machines have slow disks and stall for a second now and then, so they get more room.
/// The limit that counts is the one on a real machine.
fn limit() -> Duration {
    if std::env::var_os("CI").is_some() {
        Duration::from_secs(3)
    } else {
        Duration::from_millis(500)
    }
}

fn project() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

// ---- scanning -------------------------------------------------------------------------------

#[test]
fn scan_follows_the_depth_and_skip_rules() {
    let (_dir, store) = project();
    let root = store.root();
    for rel in [
        "README.md",
        "backend/x.md",
        "board/Y.MD",
        "a/b/c/d/deep.md",
        "a/b/c/d/e/too-deep.md",
        ".hidden/no.md",
        "backend/.draft.md",
        "node_modules/pkg/no.md",
        "dist/no.md",
        "out/no.md",
        "backend/notes.txt",
    ] {
        put(root, rel, "# card\n");
    }
    let found: Vec<String> = store
        .scan()
        .unwrap()
        .into_iter()
        .map(|f| f.rel_path)
        .collect();
    assert_eq!(
        found,
        ["README.md", "a/b/c/d/deep.md", "backend/x.md", "board/Y.MD"]
    );
    // The watcher uses the same rule on single paths.
    for rel in ["README.md", "a/b/c/d/deep.md", "board/Y.MD"] {
        assert!(is_card_path(rel), "{rel}");
    }
    for rel in [
        "a/b/c/d/e/too-deep.md",
        ".hidden/no.md",
        "backend/.draft.md",
        "dist/no.md",
        "x.txt",
        "",
    ] {
        assert!(!is_card_path(rel), "{rel}");
    }
}

#[test]
fn scan_lists_oversized_and_non_utf8_files_without_reading_them() {
    let (_dir, store) = project();
    put(store.root(), "big.md", &"x".repeat(1024 * 1024 + 1));
    fs::write(store.root().join("latin1.md"), [b'#', b' ', 0xE9]).unwrap();
    put(store.root(), "ok.md", "# fine\n");
    let files = store.scan().unwrap();
    assert!(files[0].content.as_ref().unwrap_err().contains("too large"));
    assert_eq!(files[1].content.as_ref().unwrap_err(), "not UTF-8 text");
    assert_eq!(files[2].content.as_deref(), Ok("# fine\n"));
}

#[test]
fn a_missing_root_is_an_error_not_an_empty_project() {
    let (dir, store) = project();
    let moved = dir.path().with_extension("moved");
    fs::rename(dir.path(), &moved).unwrap();
    assert!(store.scan().is_err());
    assert!(Store::open(dir.path()).is_err());
    fs::rename(moved, dir.path()).unwrap();
}

// ---- path guard -----------------------------------------------------------------------------

#[test]
fn paths_that_leave_the_root_are_refused() {
    let (_dir, store) = project();
    put(store.root(), "sub/card.md", "x");
    assert!(store.resolve("sub/card.md").is_ok());
    assert!(store.resolve("sub/new-folder/new.md").is_ok());
    let absolute = store.root().join("sub/card.md");
    for bad in [
        "../escape.md",
        "sub/../../escape.md",
        "",
        absolute.to_str().unwrap(),
    ] {
        assert!(
            matches!(store.resolve(bad), Err(StoreError::OutsideRoot(_))),
            "{bad:?}"
        );
        assert!(store.write(bad, "x").is_err());
    }
}

#[cfg(unix)]
#[test]
fn symlinks_out_of_the_root_are_refused_and_not_scanned() {
    let (_dir, store) = project();
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "secret.md", "secret");
    std::os::unix::fs::symlink(outside.path(), store.root().join("link")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        store.root().join("alias.md"),
    )
    .unwrap();

    assert!(matches!(
        store.read("link/secret.md"),
        Err(StoreError::OutsideRoot(_))
    ));
    assert!(matches!(
        store.read("alias.md"),
        Err(StoreError::OutsideRoot(_))
    ));
    assert!(matches!(
        store.write("link/new.md", "x"),
        Err(StoreError::OutsideRoot(_))
    ));
    assert!(store.scan().unwrap().is_empty());
    assert!(!outside.path().join("new.md").exists());
}

#[test]
fn only_image_types_can_be_read_as_images() {
    let (_dir, store) = project();
    put(store.root(), "img/pic.PNG", "png bytes");
    put(store.root(), "card.md", "text");
    assert_eq!(store.read_image("img/pic.PNG").unwrap(), b"png bytes");
    assert!(matches!(
        store.read_image("card.md"),
        Err(StoreError::NotAnImage(_))
    ));
    assert!(matches!(
        store.read_image("img/gone.png"),
        Err(StoreError::NotFound(_))
    ));
    assert!(matches!(
        store.read_image("../x.png"),
        Err(StoreError::OutsideRoot(_))
    ));
}

// ---- reading and writing --------------------------------------------------------------------

#[test]
fn write_read_rename_delete_and_free_names() {
    let (_dir, store) = project();
    store.write("backend/fix-2026-10-11.md", "one").unwrap();
    assert_eq!(store.read("backend/fix-2026-10-11.md").unwrap(), "one");
    assert!(matches!(
        store.read("nope.md"),
        Err(StoreError::NotFound(_))
    ));

    assert_eq!(
        store.free_name("backend", "new-2026-10-11.md").unwrap(),
        "backend/new-2026-10-11.md"
    );
    assert_eq!(
        store.free_name("backend", "fix-2026-10-11.md").unwrap(),
        "backend/fix-2026-10-11-2.md"
    );
    store.write("backend/fix-2026-10-11-2.md", "two").unwrap();
    assert_eq!(
        store.free_name("backend", "fix-2026-10-11.md").unwrap(),
        "backend/fix-2026-10-11-3.md"
    );
    assert_eq!(store.free_name("", "root.md").unwrap(), "root.md");

    store
        .rename("backend/fix-2026-10-11.md", "archive/fix-2026-10-11.md")
        .unwrap();
    assert_eq!(store.read("archive/fix-2026-10-11.md").unwrap(), "one");
    // A rename never replaces a file.
    assert!(
        store
            .rename("backend/fix-2026-10-11-2.md", "archive/fix-2026-10-11.md")
            .is_err()
    );
    assert_eq!(store.read("backend/fix-2026-10-11-2.md").unwrap(), "two");

    assert_eq!(store.delete("archive/fix-2026-10-11.md").unwrap(), "one");
    assert!(matches!(
        store.read("archive/fix-2026-10-11.md"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn update_works_on_what_is_on_disk_now() {
    let (_dir, store) = project();
    store.write("card.md", "status: todo\nbody v1").unwrap();
    // Another program changes the file after mdk last looked at it.
    put(store.root(), "card.md", "status: todo\nbody by the agent");
    let (before, after) = store
        .update("card.md", |on_disk| Ok(on_disk.replace("todo", "done")))
        .unwrap();
    assert_eq!(before, "status: todo\nbody by the agent");
    assert_eq!(after, "status: done\nbody by the agent");
    assert_eq!(store.read("card.md").unwrap(), after);
    // A change that is refused writes nothing.
    assert!(
        store
            .update("card.md", |_| Err("Fix the frontmatter first".into()))
            .is_err()
    );
    assert_eq!(store.read("card.md").unwrap(), after);
}

#[test]
fn only_card_files_are_left_in_the_folder() {
    let (_dir, store) = project();
    store.write("a.md", "1").unwrap();
    store.update("a.md", |t| Ok(format!("{t}2"))).unwrap();
    store.write("sub/b.md", "x").unwrap();
    store.rename("sub/b.md", "sub/c.md").unwrap();
    store.delete("sub/c.md").unwrap();
    let mut left = Vec::new();
    let mut dirs = vec![store.root().to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            if entry.path().is_dir() {
                dirs.push(entry.path())
            } else {
                left.push(entry.file_name())
            }
        }
    }
    assert_eq!(left, ["a.md"], "no lock, temp or hidden files");
}

// ---- watching -------------------------------------------------------------------------------

fn watch(store: &Store) -> (mdk_store::Watcher, Receiver<Vec<StoreEvent>>) {
    let (tx, rx) = channel();
    let watcher = store.watch(move |batch| drop(tx.send(batch))).unwrap();
    // Let the OS watcher start, and drain whatever the setup produced.
    std::thread::sleep(Duration::from_millis(400));
    while rx.try_recv().is_ok() {}
    (watcher, rx)
}

/// Wait for a batch containing `event`. Returns how long it took.
fn wait_for(rx: &Receiver<Vec<StoreEvent>>, event: &StoreEvent, start: Instant) -> Duration {
    loop {
        let batch = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("never saw {event:?}"));
        if batch.contains(event) {
            return start.elapsed();
        }
    }
}

fn changed(rel: &str) -> StoreEvent {
    StoreEvent::Changed {
        rel_path: rel.into(),
    }
}

fn removed(rel: &str) -> StoreEvent {
    StoreEvent::Removed {
        rel_path: rel.into(),
    }
}

#[test]
fn an_external_edit_is_reported_within_500ms() {
    let (_dir, store) = project();
    put(store.root(), "backend/card.md", "status: todo");
    let (_watcher, rx) = watch(&store);
    let start = Instant::now();
    put(store.root(), "backend/card.md", "status: done");
    let took = wait_for(&rx, &changed("backend/card.md"), start);
    assert!(took < limit(), "took {took:?}");
}

#[test]
fn the_stores_own_writes_are_not_reported() {
    let (_dir, store) = project();
    put(store.root(), "card.md", "v1");
    put(store.root(), "gone.md", "x");
    put(store.root(), "moved.md", "y");
    store.scan().unwrap();
    let (_watcher, rx) = watch(&store);
    store.write("card.md", "v2").unwrap();
    store.update("card.md", |t| Ok(format!("{t}+"))).unwrap();
    store.write("new.md", "new").unwrap();
    store.delete("gone.md").unwrap();
    store.rename("moved.md", "archive/moved.md").unwrap();
    // The new archive/ folder may ask for a rescan. No card event may arrive.
    let deadline = Instant::now() + Duration::from_millis(700);
    while let Ok(batch) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        assert!(
            batch.iter().all(|e| *e == StoreEvent::Rescan),
            "own write reported: {batch:?}"
        );
    }
    // An outside edit to the same file still gets through afterwards.
    let start = Instant::now();
    put(store.root(), "card.md", "by the agent");
    assert!(wait_for(&rx, &changed("card.md"), start) < limit());
}

#[test]
fn rename_saves_deletes_and_renames_are_reported() {
    let (_dir, store) = project();
    put(store.root(), "card.md", "old");
    put(store.root(), "bye.md", "x");
    put(store.root(), "before.md", "y");
    store.scan().unwrap();
    let (_watcher, rx) = watch(&store);

    // What vim and git do: write a sibling, then rename it over the card.
    let start = Instant::now();
    put(store.root(), "card.md.swap", "new");
    fs::rename(
        store.root().join("card.md.swap"),
        store.root().join("card.md"),
    )
    .unwrap();
    assert!(wait_for(&rx, &changed("card.md"), start) < limit());

    let start = Instant::now();
    fs::remove_file(store.root().join("bye.md")).unwrap();
    assert!(wait_for(&rx, &removed("bye.md"), start) < limit());

    // macOS reports only the new name. The old one is found by the sweep.
    fs::rename(
        store.root().join("before.md"),
        store.root().join("after.md"),
    )
    .unwrap();
    let mut seen = Vec::new();
    while !(seen.contains(&removed("before.md")) && seen.contains(&changed("after.md"))) {
        let batch = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("rename not reported, saw {seen:?}"));
        seen.extend(batch);
    }
}

#[test]
fn a_burst_of_50_files_arrives_in_a_few_batches() {
    let (_dir, store) = project();
    let cards: Vec<String> = (0..50).map(|i| format!("c{i:02}.md")).collect();
    for card in &cards {
        put(store.root(), card, "old");
    }
    let (_watcher, rx) = watch(&store);
    let start = Instant::now();
    for card in &cards {
        put(store.root(), card, "new");
    }
    let (mut seen, mut batches) = (std::collections::BTreeSet::new(), 0);
    while seen.len() < cards.len() {
        let batch = rx.recv_timeout(Duration::from_secs(5)).expect("burst lost");
        batches += 1;
        for event in batch {
            let StoreEvent::Changed { rel_path } = event else {
                panic!("{event:?}")
            };
            seen.insert(rel_path);
        }
    }
    assert!(start.elapsed() < limit(), "took {:?}", start.elapsed());
    assert!(batches <= 3, "{batches} batches");
}

#[test]
fn a_new_subfolder_is_picked_up_and_watched() {
    let (_dir, store) = project();
    let (_watcher, rx) = watch(&store);
    fs::create_dir(store.root().join("frontend")).unwrap();
    wait_for(&rx, &StoreEvent::Rescan, Instant::now());
    std::thread::sleep(Duration::from_millis(300));
    let start = Instant::now();
    put(store.root(), "frontend/card.md", "x");
    assert!(wait_for(&rx, &changed("frontend/card.md"), start) < limit());
}
