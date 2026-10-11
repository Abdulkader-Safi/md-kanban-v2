//! NFR-01 and NFR-04 at the board level: a 1,000-card project scans fast and filters
//! well inside 100 ms. Run with `--nocapture` to see the timings.

use mdk::board::{Board, Filters};
use mdk_format::{Date, Status};
use std::fs;
use std::time::{Duration, Instant};

#[test]
fn a_thousand_cards_scan_and_filter_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let statuses = ["backlog", "todo", "in-progress", "review", "done"];
    for i in 0..1000 {
        let folder = dir.path().join(format!("work-{}", i % 8));
        fs::create_dir_all(&folder).unwrap();
        let text = format!(
            "---\nid: \"card-{i}\"\nstatus: \"{}\"\npriority: \"high\"\nassignee: \"safi\"\ndueDate: null\nlabels: [\"bench\", \"work-{}\"]\norder: {i}\n---\n# Card {i}\n\nNotes for card {i}. Some text so the card has an excerpt.\n\n- [ ] one\n- [x] two\n",
            statuses[i % 5],
            i % 8
        );
        fs::write(folder.join(format!("card-{i}.md")), text).unwrap();
    }
    let mut board = Board::default();
    let id = board.connect_project(dir.path()).unwrap();
    board.today = Some(Date {
        year: 2026,
        month: 10,
        day: 11,
    });

    let start = Instant::now();
    board.scan_now(&id);
    let scan = start.elapsed();
    assert_eq!(board.card_count(), 1000);

    // What one keystroke in the search box costs: filter, sort and split into columns.
    let start = Instant::now();
    board.filters = Filters {
        query: "card 99".into(),
        ..Default::default()
    };
    let hits = board.visible_cards().len();
    let columns: usize = Status::ALL.into_iter().map(|s| board.column(s).len()).sum();
    let filter = start.elapsed();
    assert_eq!((hits, columns), (11, 11), "card 99 and card 990 to 999");

    board.filters = Filters::default();
    let start = Instant::now();
    let all: usize = Status::ALL.into_iter().map(|s| board.column(s).len()).sum();
    let unfiltered = start.elapsed();
    assert_eq!(all, 1000);

    println!(
        "scan 1000 cards: {scan:?}, filter keystroke: {filter:?}, all columns: {unfiltered:?}"
    );
    assert!(
        filter < Duration::from_millis(100),
        "filtering took {filter:?}"
    );
    assert!(
        unfiltered < Duration::from_millis(100),
        "building the columns took {unfiltered:?}"
    );
    assert!(scan < Duration::from_secs(5), "the scan took {scan:?}");
}
