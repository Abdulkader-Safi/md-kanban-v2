//! The parts of the preview that are plain text work: rewriting Obsidian syntax into
//! markdown the parser knows, resolving links to cards, and reading callout headers.
//!
//! Wikilinks and `==highlight==` sit in the middle of text, where a parser plugin cannot
//! reach them, so they are rewritten into links with an `mdk-…:` scheme before parsing.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// A wikilink or relative link that leads to a card.
pub const CARD_SCHEME: &str = "mdk-card:";
/// A wikilink or relative link with nothing behind it.
pub const MISSING_SCHEME: &str = "mdk-missing:";
/// `==highlighted==` text.
pub const HIGHLIGHT_SCHEME: &str = "mdk-hl:";

const IMAGE_TYPES: [&str; 9] = [
    "png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "bmp", "ico",
];

/// What the preview needs to know about a card to link to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardRef {
    pub id: String,
    pub title: String,
    /// Relative to the project root, with `/` separators.
    pub rel_path: String,
}

pub fn is_image(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| IMAGE_TYPES.contains(&ext.to_lowercase().as_str()))
}

pub fn is_external(href: &str) -> bool {
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| href.starts_with(scheme))
}

/// `%20` and friends. A stray `%` is left as it is.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
            Some(byte) if bytes[i] == b'%' => {
                out.push(byte);
                i += 3;
            }
            _ => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Resolve a link written in the card at `page_path` to a path in the project. A leading
/// `/` is the project root. `None` when the link climbs out of the project.
pub fn resolve_path(page_path: &str, href: &str) -> Option<String> {
    let href = percent_decode(href.split('#').next().unwrap_or(""));
    let dir = page_path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let joined = match href.strip_prefix('/') {
        Some(from_root) => from_root.to_string(),
        None => format!("{dir}/{href}"),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The card a relative link points at: the path as written, or with `.md` added.
pub fn find_link<'a>(cards: &'a [CardRef], page_path: &str, href: &str) -> Option<&'a CardRef> {
    let path = resolve_path(page_path, href)?;
    let with_md = format!("{path}.md");
    cards
        .iter()
        .find(|c| c.rel_path == path || c.rel_path == with_md)
}

/// The card a `[[wikilink]]` names: by title, id, file name or path, ignoring case and
/// any `#heading`.
pub fn find_wikilink<'a>(cards: &'a [CardRef], target: &str) -> Option<&'a CardRef> {
    let wanted = target.split('#').next().unwrap_or("").trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let stem = |path: &str| path.strip_suffix(".md").unwrap_or(path).to_lowercase();
    cards.iter().find(|card| {
        let file = card.rel_path.rsplit('/').next().unwrap_or(&card.rel_path);
        card.title.to_lowercase() == wanted
            || card.id.to_lowercase() == wanted
            || stem(file) == wanted
            || stem(&card.rel_path) == wanted
    })
}

/// The dollar guard: `$x$` is maths only when there is no space just inside the dollars
/// and no digit right after the closing one. "It costs $5 a month, or $50" stays text.
pub fn dollar_guard(value: &str, after: Option<char>) -> bool {
    !value.is_empty()
        && !value.starts_with(char::is_whitespace)
        && !value.ends_with(char::is_whitespace)
        && !after.is_some_and(|c| c.is_ascii_digit())
}

// ---- callouts -------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Note,
    Tip,
    Warning,
    Danger,
    Quote,
}

impl Tone {
    /// Unknown types are notes.
    pub fn of(kind: &str) -> Tone {
        match kind {
            "tip" | "hint" | "success" | "check" | "done" => Tone::Tip,
            "warning" | "caution" | "attention" | "question" | "help" | "faq" => Tone::Warning,
            "danger" | "error" | "failure" | "fail" | "missing" | "bug" => Tone::Danger,
            "quote" | "cite" | "example" => Tone::Quote,
            _ => Tone::Note,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callout {
    pub tone: Tone,
    /// The custom title, or the type in capitals ("NOTE").
    pub label: String,
    /// `Some(true)` for `[!x]-` (starts folded), `Some(false)` for `[!x]+`, `None` when it
    /// cannot fold.
    pub folded: Option<bool>,
    pub body: String,
}

/// Read a blockquote's source as a callout. `None` for an ordinary quote.
pub fn parse_callout(source: &str) -> Option<Callout> {
    let mut lines = source.lines().map(|line| {
        let line = line.trim_start();
        line.strip_prefix("> ")
            .or(line.strip_prefix('>'))
            .unwrap_or(line)
    });
    let (kind, after) = lines
        .next()?
        .trim_start()
        .strip_prefix("[!")?
        .split_once(']')?;
    if kind.is_empty()
        || !kind
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let (folded, title) = match after.chars().next() {
        Some('-') => (Some(true), &after[1..]),
        Some('+') => (Some(false), &after[1..]),
        _ => (None, after),
    };
    let title = title.trim();
    Some(Callout {
        tone: Tone::of(&kind.to_lowercase()),
        label: if title.is_empty() {
            kind.to_uppercase()
        } else {
            title.to_string()
        },
        folded,
        body: lines.collect::<Vec<_>>().join("\n"),
    })
}

// ---- the source rewrite ---------------------------------------------------------------------

/// Markdown ready for the parser, plus what could not be said in markdown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prepared {
    pub source: String,
    /// `title="…"` from a code fence, keyed by [`code_key`] of the block's code.
    pub code_titles: HashMap<u64, String>,
}

/// The key a code block's title is stored under.
pub fn code_key(code: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    code.trim_end().hash(&mut hasher);
    hasher.finish()
}

fn escape_link_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn url_part(text: &str) -> String {
    text.replace('%', "%25")
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

/// `rest` starts with `open`, and `close` follows on the same line with something between.
fn take<'a>(rest: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str)> {
    let (inner, after) = rest.strip_prefix(open)?.split_once(close)?;
    (!inner.is_empty() && !inner.contains('\n')).then_some((inner, after))
}

fn wikilink(inner: &str, cards: &[CardRef]) -> String {
    let (target, label) = match inner.split_once('|') {
        Some((target, label)) => (target.trim(), label.trim().to_string()),
        // [[card#Heading]] reads as "card > Heading".
        None => (inner.trim(), inner.trim().replace('#', " > ")),
    };
    let label = escape_link_text(&label);
    match find_wikilink(cards, target) {
        Some(card) => format!("[{label}]({CARD_SCHEME}{})", url_part(&card.rel_path)),
        None => format!("[{label}]({MISSING_SCHEME})"),
    }
}

/// Whether `<…` here is an HTML tag (to show as text), and not `<br>` or an autolink.
fn is_html_tag(rest: &str) -> bool {
    let after = &rest[1..];
    let lower = after.to_lowercase();
    let is_break = ["br>", "br/>", "br />"]
        .iter()
        .any(|b| lower.starts_with(b));
    let is_autolink = ["http://", "https://", "mailto:"]
        .iter()
        .any(|s| lower.starts_with(s));
    !is_break
        && !is_autolink
        && after.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!')
}

fn inline(line: &str, out: &mut String, cards: &[CardRef]) {
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        if ch == '`' || ch == '$' {
            // Inline code and maths pass through untouched.
            let end = rest[1..].find(ch).map_or(1, |i| i + 2);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if ch == '\\' && rest.len() > 1 {
            // An escaped character stays escaped.
            let end = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if let Some((inner, after)) = take(rest, "![[", "]]") {
            let (file, size) = inner.split_once('|').unwrap_or((inner, ""));
            if is_image(file.trim()) {
                let name = file.trim();
                out.push_str(&format!(
                    "![{}]({} \"{}\")",
                    escape_link_text(name),
                    url_part(name),
                    size.trim().replace('"', "")
                ));
            } else {
                // Embedding a card is a link to it.
                out.push_str(&wikilink(inner, cards));
            }
            rest = after;
        } else if let Some((inner, after)) = take(rest, "[[", "]]") {
            out.push_str(&wikilink(inner, cards));
            rest = after;
        } else if let Some((inner, after)) = take(rest, "==", "==")
            .filter(|(inner, _)| !inner.starts_with(' ') && !inner.ends_with(' '))
        {
            out.push_str(&format!(
                "[{}]({HIGHLIGHT_SCHEME})",
                escape_link_text(inner)
            ));
            rest = after;
        } else if ch == '<' && is_html_tag(rest) {
            // Raw HTML is shown as text, never rendered.
            out.push_str("\\<");
            rest = &rest[1..];
        } else {
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
}

/// Rewrite a card body for the preview: wikilinks, embeds and highlights become links the
/// plugins understand, raw HTML is escaped, and code fence titles are set aside.
/// Fenced code, inline code and maths are left exactly as written.
pub fn prepare(markdown: &str, cards: &[CardRef]) -> Prepared {
    let mut prepared = Prepared::default();
    let mut out = String::with_capacity(markdown.len());
    // The open fence, and for a titled code fence its title and the code so far.
    let mut fence: Option<&str> = None;
    let mut titled: Option<(String, String)> = None;

    for line in markdown.split_inclusive('\n') {
        let t = line.trim_start();
        match fence {
            Some(f) => {
                if t.starts_with(f) {
                    fence = None;
                    if let Some((title, code)) = titled.take() {
                        prepared.code_titles.insert(code_key(&code), title);
                    }
                } else if let Some((_, code)) = &mut titled {
                    code.push_str(line);
                }
                out.push_str(line);
            }
            None if t.starts_with("```") || t.starts_with("~~~") => {
                fence = Some(&t[..3]);
                let info = t[3..].trim();
                match take(
                    info.split_once("title=").map_or("", |(_, rest)| rest),
                    "\"",
                    "\"",
                ) {
                    Some((title, _)) => {
                        // Keep the language only, so the highlighter still finds it.
                        titled = Some((title.to_string(), String::new()));
                        let indent = &line[..line.len() - t.len()];
                        let lang = info.split_whitespace().next().unwrap_or("");
                        let lang = if lang.starts_with("title=") { "" } else { lang };
                        out.push_str(&format!("{indent}{}{lang}\n", &t[..3]));
                    }
                    None => out.push_str(line),
                }
            }
            None if t.starts_with("$$") => {
                // A one-line `$$…$$` opens and closes here.
                if !t[2..].trim_end().ends_with("$$") || t.trim_end().len() < 4 {
                    fence = Some("$$");
                }
                out.push_str(line);
            }
            None => inline(line, &mut out, cards),
        }
    }
    prepared.source = out;
    prepared
}
