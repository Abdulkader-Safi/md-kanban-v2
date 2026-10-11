//! Every fixture card is parsed and written, and compared with what the Electron app's own
//! `parseTaskFile` and `stringifyTaskFile` produced (see `fixtures/generate.ts`).

use mdk_format::{ParsedCard, parse, write};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// The frozen clock `generate.ts` ran under.
const NOW: &str = "2026-01-02T03:04:05.678Z";

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files: Vec<PathBuf> = ["cards", "edge"]
        .iter()
        .flat_map(|dir| fs::read_dir(root.join(dir)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    assert!(files.len() >= 46, "fixtures are missing");
    files
}

fn load(path: &Path) -> (String, ParsedCard) {
    let raw = fs::read_to_string(path).unwrap();
    let mut card = parse(&raw, path.file_name().unwrap().to_str().unwrap());
    // A missing timestamp defaults to the current time, which the fixtures froze.
    for (key, field) in [
        ("created:", &mut card.created),
        ("modified:", &mut card.modified),
    ] {
        if !raw
            .lines()
            .any(|line| line.starts_with(key) && line.len() > key.len())
        {
            *field = NOW.to_string();
        }
    }
    (raw, card)
}

#[test]
fn parses_every_fixture_like_electron() {
    for path in fixtures() {
        let (_, card) = load(&path);
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(path.with_extension("json")).unwrap())
                .unwrap();
        let actual = json!({
            "id": card.id,
            "title": card.title,
            "body": card.body,
            "status": card.status.as_str(),
            "priority": card.priority.as_str(),
            "assignee": card.assignee,
            "dueDate": card.due_date,
            "created": card.created,
            "modified": card.modified,
            "labels": card.labels,
            "order": card.order,
        });
        // 3 and 3.0 are the same number to Electron.
        let order = |v: &Value| v["order"].as_f64();
        assert_eq!(order(&actual), order(&expected), "{}", path.display());
        let strip = |mut v: Value| {
            v.as_object_mut().unwrap().remove("order");
            v
        };
        assert_eq!(strip(actual), strip(expected), "{}", path.display());
    }
}

#[test]
fn writes_every_fixture_like_electron() {
    for path in fixtures() {
        let (_, mut card) = load(&path);
        // Electron drops unknown keys. Without them, the output must match byte for byte.
        card.extra.clear();
        let expected = fs::read_to_string(path.with_extension("written")).unwrap();
        assert_eq!(write(&card, NOW), expected, "{}", path.display());
    }
}

#[test]
fn round_trip_is_stable() {
    for path in fixtures() {
        let (_, card) = load(&path);
        let name = path.file_name().unwrap().to_str().unwrap();
        let once = write(&card, NOW);
        let again = parse(&once, name);
        assert_eq!(write(&again, NOW), once, "{}", path.display());
        // The nine fields and the body survive a write. The body gains a heading if it had none.
        let mut expected = card.clone();
        if !again.body.eq(&card.body) {
            expected.body = format!("# {}\n\n{}", card.title, card.body)
                .trim()
                .to_string();
        }
        expected.modified = NOW.to_string();
        expected.parse_error = None;
        assert_eq!(
            ParsedCard {
                parse_error: None,
                ..again
            },
            expected,
            "{}",
            path.display()
        );
    }
}
