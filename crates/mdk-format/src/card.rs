//! Reading and writing card files. Ports of Electron's `markdown.ts`.

use crate::{FormatError, LineEnding, Priority, Status};
use regex::Regex;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

static KEY_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z0-9_-]+)\s*:\s*(.*)$").unwrap());
static DASH_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*-\s+(.*)$").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#\s+(.+)$").unwrap());

/// The nine keys mdk owns, in the order they are written.
const KNOWN: [&str; 9] = [
    "id", "status", "priority", "assignee", "dueDate", "created", "modified", "labels", "order",
];

/// A frontmatter key mdk does not know, kept as the exact lines it was written in.
/// `name` is empty for comment lines that sit above the first key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawKey {
    pub name: String,
    pub source_lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedCard {
    pub id: String,
    pub title: String,
    pub body: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: String,
    pub due_date: Option<String>,
    pub created: String,
    pub modified: String,
    pub labels: Vec<String>,
    pub order: f64,
    pub extra: Vec<RawKey>,
    pub line_ending: LineEnding,
    /// Set when the frontmatter is not valid YAML. Such a file is shown but never rewritten.
    pub parse_error: Option<String>,
}

enum Value {
    Null,
    Num(f64),
    Str(String),
    List(Vec<String>),
}

impl Value {
    fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Electron's `asText`: strings as is, numbers printed, everything else empty.
    fn text(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Num(n) => number(*n),
            _ => String::new(),
        }
    }
}

/// A number the way JavaScript prints it: `3`, `2.5`, `1728650000000`.
fn number(n: f64) -> String {
    if n == 0.0 { "0".into() } else { n.to_string() }
}

fn quote(s: &str) -> String {
    // serde_json escapes exactly what JSON.stringify does, and keeps non-ASCII as is.
    serde_json::to_string(s).expect("a string always serialises")
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    let quoted = |q: char| t.len() >= 2 && t.starts_with(q) && t.ends_with(q);
    if quoted('"') {
        serde_json::from_str::<String>(t).unwrap_or_else(|_| {
            t[1..t.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
        })
    } else if quoted('\'') {
        t[1..t.len() - 1].replace("''", "'")
    } else {
        t.to_string()
    }
}

fn scalar(s: &str) -> Value {
    let v = s.trim();
    if v.is_empty() || v == "null" || v == "~" {
        Value::Null
    } else if NUMBER.is_match(v) {
        Value::Num(v.parse().unwrap_or(0.0))
    } else {
        Value::Str(unquote(v))
    }
}

fn inline_array(v: &str) -> Vec<String> {
    let t = v.trim();
    // Everything between the first `[` and the last `]`. With no `]`, the last character goes,
    // which is what `slice(1, -1)` does in the original.
    let end = t
        .rfind(']')
        .or_else(|| t.char_indices().next_back().map(|(i, _)| i));
    let inner = end.filter(|&end| end >= 1).map_or("", |end| &t[1..end]);

    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut previous = '\0';
    for ch in inner.chars() {
        match quote {
            Some(q) => {
                current.push(ch);
                if ch == q && previous != '\\' {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            None if ch == ',' => out.push(unquote(&std::mem::take(&mut current))),
            None => current.push(ch),
        }
        previous = ch;
    }
    out.push(unquote(&current));
    out.retain(|item| !item.is_empty());
    out
}

/// Electron's `splitFrontmatter`, on text that already has `\n` line endings and no BOM.
/// Returns the frontmatter lines and the body, or `None` when there is no frontmatter.
fn split(text: &str) -> Option<(Vec<&str>, String)> {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines[0].trim() != "---" {
        return None;
    }
    let end = (1..lines.len()).find(|&i| lines[i].trim() == "---")?;
    Some((lines[1..end].to_vec(), lines[end + 1..].join("\n")))
}

/// Electron's `parseFront`, which also collects the lines of every key mdk does not own.
fn front(lines: &[&str]) -> (Vec<(String, Value)>, Vec<RawKey>) {
    let mut data: Vec<(String, Value)> = Vec::new();
    let mut extra: Vec<RawKey> = Vec::new();
    // Whether the lines being read belong to an unknown key (or sit above the first key).
    let mut in_extra = true;
    let mut block: Option<Vec<String>> = None;

    fn flush(data: &mut [(String, Value)], block: &mut Option<Vec<String>>) {
        if let (Some(items), Some(last)) = (block.take(), data.last_mut()) {
            last.1 = Value::List(items);
        }
    }

    for line in lines {
        let key = KEY_LINE.captures(line);
        let skipped = line.trim().is_empty() || line.trim().starts_with('#');
        match key {
            Some(kv) if !skipped && !(DASH_LINE.is_match(line) && !data.is_empty()) => {
                flush(&mut data, &mut block);
                let (name, rest) = (kv[1].to_string(), kv[2].trim());
                in_extra = !KNOWN.contains(&name.as_str());
                if in_extra {
                    extra.push(RawKey {
                        name: name.clone(),
                        source_lines: vec![line.to_string()],
                    });
                }
                let value = if rest.is_empty() {
                    Value::Str(String::new())
                } else if rest.starts_with('[') {
                    Value::List(inline_array(rest))
                } else {
                    scalar(rest)
                };
                data.push((name, value));
            }
            _ => {
                if in_extra {
                    // A continuation of an unknown key, or a comment above the first key.
                    match extra.last_mut() {
                        Some(raw) => raw.source_lines.push(line.to_string()),
                        None if !line.trim().is_empty() => extra.push(RawKey {
                            name: String::new(),
                            source_lines: vec![line.to_string()],
                        }),
                        None => {}
                    }
                } else if line.trim().starts_with('#') {
                    // A comment among the known keys. Keep it, or the next save would drop it.
                    extra.push(RawKey {
                        name: String::new(),
                        source_lines: vec![line.to_string()],
                    });
                }
                if skipped || data.is_empty() {
                    continue;
                }
                if let Some(dash) = DASH_LINE.captures(line) {
                    let item = unquote(&dash[1]);
                    let items = block.get_or_insert_with(Vec::new);
                    if !item.is_empty() {
                        items.push(item);
                    }
                }
            }
        }
    }
    flush(&mut data, &mut block);
    (data, extra)
}

/// The current UTC time the way JavaScript's `toISOString()` prints it.
pub fn now_iso() -> String {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since.as_secs();
    let date = crate::Date::from_days((secs / 86_400) as i64);
    let (h, m, s) = (secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    format!("{date}T{h:02}:{m:02}:{s:02}.{:03}Z", since.subsec_millis())
}

/// Parse one card file. `file_name` supplies the id when the frontmatter has none.
pub fn parse(raw: &str, file_name: &str) -> ParsedCard {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let line_ending = if text.contains("\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    };
    // The original only understands `\n`. Normalising first makes CRLF files parse the same.
    let text = text.replace("\r\n", "\n");

    let (front_lines, body) = split(&text).unwrap_or((Vec::new(), text.clone()));
    let (data, extra) = front(&front_lines);
    let get = |key: &str| data.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v);
    let text_of = |key: &str| get(key).map(Value::text).unwrap_or_default();

    let parse_error = serde_saphyr::from_str::<serde_json::Value>(&front_lines.join("\n"))
        .err()
        .filter(|_| front_lines.iter().any(|l| !l.trim().is_empty()))
        .map(|e| e.to_string());

    let id = get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            file_name
                .strip_suffix(".md")
                .unwrap_or(file_name)
                .to_string()
        });
    let body = body.trim().to_string();
    let title = TITLE
        .captures(&body)
        .map_or(id.as_str(), |m| m.get(1).unwrap().as_str())
        .trim()
        .to_string();
    let or_now = |s: String| if s.is_empty() { now_iso() } else { s };

    ParsedCard {
        title,
        body,
        status: get("status")
            .and_then(Value::as_str)
            .and_then(Status::parse)
            .unwrap_or(Status::Backlog),
        priority: get("priority")
            .and_then(Value::as_str)
            .and_then(Priority::parse)
            .unwrap_or(Priority::Medium),
        assignee: text_of("assignee"),
        due_date: Some(text_of("dueDate")).filter(|d| !d.is_empty()),
        created: or_now(text_of("created")),
        modified: or_now(text_of("modified")),
        labels: match get("labels") {
            Some(Value::List(items)) => items.clone(),
            Some(Value::Str(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        },
        order: match get("order") {
            Some(Value::Num(n)) => *n,
            _ => 0.0,
        },
        id,
        extra,
        line_ending,
        parse_error,
    }
}

/// Write a card in the Electron format, with the unknown keys after the nine known ones.
/// `modified` is the timestamp to stamp, normally [`now_iso`].
pub fn write(card: &ParsedCard, modified: &str) -> String {
    let labels: Vec<String> = card.labels.iter().map(|l| quote(l)).collect();
    let mut lines = vec![
        "---".to_string(),
        format!("id: {}", quote(&card.id)),
        format!("status: {}", quote(card.status.as_str())),
        format!("priority: {}", quote(card.priority.as_str())),
        format!("assignee: {}", quote(&card.assignee)),
        match card.due_date.as_deref().filter(|d| !d.is_empty()) {
            Some(due) => format!("dueDate: {}", quote(due)),
            None => "dueDate: null".to_string(),
        },
        format!("created: {}", quote(&card.created)),
        format!("modified: {}", quote(modified)),
        format!("labels: [{}]", labels.join(", ")),
        format!("order: {}", number(card.order)),
    ];
    lines.extend(
        card.extra
            .iter()
            .flat_map(|raw| raw.source_lines.iter().cloned()),
    );
    // A blank line that trailed the last unknown key would sit before the closing fence.
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.push("---".to_string());

    let body = if TITLE.is_match(&card.body) {
        card.body.clone()
    } else {
        format!("# {}\n\n{}", card.title, card.body)
    };
    let text = format!("{}\n{}\n", lines.join("\n"), body.trim());
    match card.line_ending {
        LineEnding::Lf => text,
        LineEnding::CrLf => text.replace('\n', "\r\n"),
    }
}

/// The fields a user action can change. `None` leaves the field as it is on disk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub assignee: Option<String>,
    pub due_date: Option<Option<String>>,
    pub labels: Option<Vec<String>>,
    pub order: Option<f64>,
}

/// Apply `patch` to the file text just read from disk, and return the new file text.
/// Everything the patch does not name stays as it is on disk, so an agent's edit survives.
pub fn apply_patch(
    raw: &str,
    file_name: &str,
    patch: &CardPatch,
    modified: &str,
) -> Result<String, FormatError> {
    let mut card = parse(raw, file_name);
    if let Some(error) = card.parse_error {
        return Err(FormatError::BrokenFrontmatter(error));
    }
    if let Some(body) = &patch.body {
        card.body = body.replace("\r\n", "\n").trim().to_string();
    }
    if let Some(title) = &patch.title {
        // The title lives in the body's first `# ` heading.
        card.title = title.trim().to_string();
        let heading = format!("# {}", card.title);
        card.body = TITLE
            .replace(&card.body, regex::NoExpand(&heading))
            .into_owned();
    }
    macro_rules! set {
        ($($field:ident),*) => { $( if let Some(v) = &patch.$field { card.$field = v.clone(); } )* };
    }
    set!(status, priority, assignee, due_date, labels, order);
    Ok(write(&card, modified))
}
