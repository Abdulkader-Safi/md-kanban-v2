//! Small text helpers ported from Electron: quick-add, templates, due labels,
//! Obsidian comments, subtasks and file names.

use crate::Priority;
use regex::Regex;
use std::sync::LazyLock;

static NON_SLUG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ {0,3}(`{3,}|~{3,})").unwrap());
static SUBTASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*[-*]\s+\[)([ xX])(\]\s*.*)$").unwrap());

// ---- dates ----------------------------------------------------------------------------------

/// A calendar date. Enough date maths for due labels and quick-add, with no time zone in it:
/// the caller decides what "today" is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// Parse `YYYY-MM-DD`.
    pub fn parse(s: &str) -> Option<Date> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let (year, month, day) = (
            s[..4].parse().ok()?,
            s[5..7].parse().ok()?,
            s[8..].parse().ok()?,
        );
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Date { year, month, day })
    }

    /// Days since 1970-01-01. Howard Hinnant's `days_from_civil`.
    pub fn days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = (i64::from(self.month) + 9) % 12;
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        Date {
            year: (yoe + era * 400 + i64::from(month <= 2)) as i32,
            month: month as u32,
            day: (doy - (153 * mp + 2) / 5 + 1) as u32,
        }
    }

    pub fn add_days(self, n: i64) -> Date {
        Date::from_days(self.days() + n)
    }

    /// 0 is Sunday, like JavaScript's `getDay()`.
    pub fn weekday(self) -> i64 {
        (self.days() + 4).rem_euclid(7)
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Overdue.
    Red,
    /// Due today.
    Amber,
    Muted,
}

/// The short due label for a card: "2d overdue", "Today", "Tomorrow", "3d", or the date.
/// Whole days between dates, so the label does not drift with the time of day.
pub fn format_due(due: &str, today: Date) -> (String, Tone) {
    let Some(date) = Date::parse(due) else {
        return (due.to_string(), Tone::Muted);
    };
    match date.days() - today.days() {
        n if n < 0 => (format!("{}d overdue", -n), Tone::Red),
        0 => ("Today".into(), Tone::Amber),
        1 => ("Tomorrow".into(), Tone::Muted),
        n if n <= 30 => (format!("{n}d"), Tone::Muted),
        _ => (due.to_string(), Tone::Muted),
    }
}

// ---- quick-add ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuickAdd {
    pub title: String,
    pub priority: Option<Priority>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
}

const WEEKDAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

fn resolve_due(token: &str, today: Date) -> Option<String> {
    let t = token.to_lowercase();
    match t.as_str() {
        "today" => return Some(today.to_string()),
        "tomorrow" => return Some(today.add_days(1).to_string()),
        _ => {}
    }
    if let Some(date) = Date::parse(&t) {
        return Some(date.to_string());
    }
    let weekday = WEEKDAYS.iter().position(|w| *w == t)? as i64;
    // The next such weekday. Today's own weekday means a week from now.
    let diff = match (weekday - today.weekday()).rem_euclid(7) {
        0 => 7,
        n => n,
    };
    Some(today.add_days(diff).to_string())
}

/// Parse "Fix login p1 #backend @safi due:friday" into a title and fields.
pub fn parse_quick_add(input: &str, today: Date) -> QuickAdd {
    let mut out = QuickAdd::default();
    let mut words = Vec::new();
    for raw in input.split_whitespace() {
        let low = raw.to_lowercase();
        let priority = match low.as_str() {
            "p1" => Some(Priority::Critical),
            "p2" => Some(Priority::High),
            "p3" => Some(Priority::Medium),
            "p4" => Some(Priority::Low),
            named => Priority::parse(named),
        };
        if priority.is_some() {
            out.priority = priority;
        } else if raw.len() > 1 && raw.starts_with('#') {
            out.labels.push(raw[1..].to_string());
        } else if raw.len() > 1 && raw.starts_with('@') {
            out.assignee = Some(raw[1..].to_string());
        } else if let Some(due) = low
            .starts_with("due:")
            .then(|| resolve_due(&raw[4..], today))
            .flatten()
        {
            out.due_date = Some(due);
        } else {
            words.push(raw);
        }
    }
    out.title = words.join(" ");
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Bug,
    Feature,
    Chore,
}

/// Priority, labels and body for a New card template button.
pub fn template(kind: Template, title: &str) -> (Priority, Vec<String>, String) {
    let t = if title.is_empty() { "Untitled" } else { title };
    match kind {
        Template::Bug => (
            Priority::High,
            vec!["bug".into()],
            format!("# {t}\n\nSteps:\n- [ ] Repro\n- [ ] Fix\n- [ ] Verify\n"),
        ),
        Template::Feature => (
            Priority::Medium,
            vec!["feature".into()],
            format!("# {t}\n\n- [ ] Spec\n- [ ] Build\n- [ ] Verify\n"),
        ),
        Template::Chore => (
            Priority::Low,
            vec!["chore".into()],
            format!("# {t}\n\n- [ ] Do\n- [ ] Verify\n"),
        ),
    }
}

// ---- file names -----------------------------------------------------------------------------

/// Lowercase ASCII letters and digits, everything else collapsed to `-`, 60 characters at most.
pub fn slugify(title: &str) -> String {
    let lower = title.to_lowercase();
    let dashed = NON_SLUG.replace_all(lower.trim(), "-");
    let slug: String = dashed.trim_matches('-').chars().take(60).collect();
    if slug.is_empty() { "task".into() } else { slug }
}

/// `<slug>-YYYY-MM-DD.md`. The card id is the same without `.md`.
pub fn new_file_name(title: &str, date: Date) -> String {
    format!("{}-{date}.md", slugify(title))
}

// ---- Obsidian comments ----------------------------------------------------------------------

/// Remove `%% comments %%`, inline or across lines. Fenced code keeps its `%%`.
/// An unclosed `%%` hides the rest of the text, as Obsidian does.
// ponytail: `%%` inside inline `code` still opens a comment, same as Electron.
pub fn strip_comments(md: &str) -> String {
    if !md.contains("%%") {
        return md.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut fence = String::new();
    let mut in_comment = false;

    for line in md.split('\n') {
        let marker = FENCE.captures(line).map(|m| m[1].to_string());
        if let Some(f) = marker.filter(|_| !in_comment)
            && (fence.is_empty()
                || (f.as_bytes()[0] == fence.as_bytes()[0] && f.len() >= fence.len()))
        {
            fence = if fence.is_empty() { f } else { String::new() };
            out.push(line.to_string());
            continue;
        }
        if !fence.is_empty() {
            out.push(line.to_string());
            continue;
        }

        let mut kept = String::new();
        let mut rest = line;
        let mut touched = in_comment;
        while !rest.is_empty() {
            match (rest.find("%%"), in_comment) {
                (None, true) => break,
                (None, false) => {
                    kept.push_str(rest);
                    break;
                }
                (Some(at), true) => {
                    rest = &rest[at + 2..];
                    in_comment = false;
                }
                (Some(at), false) => {
                    kept.push_str(&rest[..at]);
                    rest = &rest[at + 2..];
                    in_comment = true;
                    touched = true;
                }
            }
        }
        // A line that held only a comment goes, so it does not split the paragraph around it.
        if touched && kept.trim().is_empty() {
            continue;
        }
        out.push(kept);
    }
    out.join("\n")
}

/// The card-face excerpt: the body as plain text, without `%% comments %%`, its title
/// heading or any markdown marks, on one line and cut to 140 characters.
pub fn excerpt(body: &str) -> String {
    static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#\s+.+$").unwrap());
    // Quote and callout marks, then a checkbox, bullet, number or heading mark.
    static LINE_MARKS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*(?:>[ \t]*)*(?:\[![^\]]*\][+-]?[ \t]*)?(?:[-*+][ \t]+\[[ xX]\][ \t]+|[-*+][ \t]+|\d+[.)][ \t]+|#{1,6}[ \t]+)?").unwrap()
    });
    // `[label](url)` and `![alt](url)` keep their words.
    static LINK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"!?\[([^\]]*)\]\([^)]*\)").unwrap());
    static MARKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*|__|==|`").unwrap());
    let visible = strip_comments(body);
    let text = TITLE.replace(&visible, "");
    let text = LINE_MARKS.replace_all(&text, "");
    let text = LINK.replace_all(&text, "$1");
    let text = MARKS.replace_all(&text, "");
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(140)
        .collect()
}

/// A line of text cut into runs, each marked true when it was inside backticks. The
/// backticks go. An unmatched backtick stays as text.
pub fn code_spans(text: &str) -> Vec<(&str, bool)> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let Some(len) = rest[open + 1..].find('`') else {
            break;
        };
        if open > 0 {
            spans.push((&rest[..open], false));
        }
        spans.push((&rest[open + 1..open + 1 + len], true));
        rest = &rest[open + 2 + len..];
    }
    if !rest.is_empty() {
        spans.push((rest, false));
    }
    spans
}

// ---- subtasks -------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subtask {
    pub done: bool,
    pub text: String,
    /// The line of the body this checkbox is on, counted from 0.
    pub line: usize,
}

/// Every `- [ ]` and `- [x]` item, in body order.
pub fn subtasks(body: &str) -> Vec<Subtask> {
    body.split('\n')
        .enumerate()
        .filter_map(|(line, text)| {
            let m = SUBTASK.captures(text)?;
            Some(Subtask {
                done: &m[2] != " ",
                text: m[3].trim_start_matches(']').trim().to_string(),
                line,
            })
        })
        .collect()
}

/// Flip the nth checkbox. An index with no checkbox leaves the body unchanged.
pub fn toggle_subtask(body: &str, index: usize) -> String {
    let mut seen = 0;
    body.split('\n')
        .map(|line| {
            let Some(m) = SUBTASK.captures(line) else {
                return line.to_string();
            };
            seen += 1;
            if seen - 1 != index {
                return line.to_string();
            }
            let mark = if &m[2] == " " { "x" } else { " " };
            format!("{}{mark}{}", &m[1], &m[3])
        })
        .collect::<Vec<_>>()
        .join("\n")
}
