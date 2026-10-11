//! Preview spike: one card through gpui-kit's TextView with mdk's plugins.
//!
//! Finding: an inline plugin replaces a whole mdast node, so it cannot pick a
//! `[[wikilink]]` or `==highlight==` out of the middle of a text node. Those two
//! are rewritten in the source before parsing (`preprocess`) into links with an
//! `mdk-*:` scheme, which a plugin on link nodes then renders. `<mark>` is no use:
//! in markdown mode the kit only pairs the `b i u s` inline tags.
//! Everything else is a plugin on a node markdown-rs already produces.

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::text::{
    InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin,
    TextView, markdown_ast::Node,
};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

/// Which custom renderers ran. The headless test reads this.
pub static SEEN: LazyLock<Mutex<BTreeSet<&'static str>>> = LazyLock::new(Default::default);

fn seen(name: &'static str) {
    SEEN.lock().unwrap().insert(name);
}

// ---- source rewrite: wikilinks, embeds, highlight -------------------------------------------

/// `rest` starts with `open`, and `close` follows on the same line with something in between.
fn take<'a>(rest: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str)> {
    let (inner, after) = rest.strip_prefix(open)?.split_once(close)?;
    (!inner.is_empty() && !inner.contains('\n')).then_some((inner, after))
}

fn inline(line: &str, out: &mut String, exists: &dyn Fn(&str) -> bool) {
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        // Inline code and maths pass through untouched.
        if ch == '`' || ch == '$' {
            let end = rest[1..].find(ch).map_or(1, |i| i + 2);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if let Some((inner, after)) = take(rest, "![[", "]]") {
            let (file, width) = inner.split_once('|').unwrap_or((inner, ""));
            out.push_str(&format!(
                "![{file}]({} \"{width}\")",
                file.replace(' ', "%20")
            ));
            rest = after;
        } else if let Some((inner, after)) = take(rest, "[[", "]]") {
            let (target, label) = inner.split_once('|').unwrap_or((inner, inner));
            let card = target.split('#').next().unwrap_or(target);
            let scheme = if exists(card) {
                "mdk-wiki"
            } else {
                "mdk-missing"
            };
            out.push_str(&format!(
                "[{label}]({scheme}:{})",
                target.replace(' ', "%20")
            ));
            rest = after;
        } else if let Some((inner, after)) = take(rest, "==", "==")
            .filter(|(inner, _)| !inner.starts_with(' ') && !inner.ends_with(' '))
        {
            out.push_str(&format!("[{inner}](mdk-hl:)"));
            rest = after;
        } else {
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
}

pub fn preprocess(src: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(src.len());
    let mut fence: Option<&str> = None;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        match fence {
            Some(f) => {
                out.push_str(line);
                if t.starts_with(f) {
                    fence = None;
                }
            }
            None if t.starts_with("```") || t.starts_with("~~~") || t.starts_with("$$") => {
                fence = Some(&t[..if t.starts_with('$') { 2 } else { 3 }]);
                out.push_str(line);
            }
            None => inline(line, &mut out, exists),
        }
    }
    out
}

// ---- maths ----------------------------------------------------------------------------------

/// Electron's dollar guard: no space just inside the dollars, no digit right after the close.
pub fn dollar_guard(value: &str, after: Option<char>) -> bool {
    !value.is_empty()
        && !value.starts_with(char::is_whitespace)
        && !value.ends_with(char::is_whitespace)
        && !after.is_some_and(|c| c.is_ascii_digit())
}

fn math(latex: &str, size: f32, window: &Window, cx: &App) -> AnyElement {
    let color = cx.theme().foreground;
    match ratex_gpui::render::render_latex(latex, size, window.scale_factor(), color) {
        Some(r) => {
            seen("math");
            img(r.image)
                .w(px(r.width))
                .h(px(r.height))
                .into_any_element()
        }
        None => {
            seen("math-error");
            div()
                .text_color(rgb(0xef4444))
                .child(latex.to_string())
                .into_any_element()
        }
    }
}

struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "mdk-inline-math"
    }

    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::InlineMath(m) = node else {
            return None;
        };
        let end = m.position.as_ref()?.end.offset;
        let after = cx.source().get(end..).and_then(|s| s.chars().next());
        dollar_guard(&m.value, after).then(|| MarkdownNode::new(self.name(), m.value.clone()))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let latex = node.data::<String>()?;
        Some(InlineElement::new(math(
            latex,
            f32::from(context.font_size()),
            window,
            cx,
        )))
    }
}

struct BlockMath;

impl MarkdownPlugin for BlockMath {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "mdk-block-math"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Math(m) = node else {
            return None;
        };
        Some(MarkdownNode::new(self.name(), m.value.clone()))
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let latex = node.data::<String>().cloned().unwrap_or_default();
        div()
            .my_3()
            .flex()
            .justify_center()
            .child(math(&latex, 20., window, cx))
    }
}

// ---- mermaid --------------------------------------------------------------------------------

type Diagram = Result<Arc<Image>, String>;
static DIAGRAMS: LazyLock<Mutex<HashMap<String, Diagram>>> = LazyLock::new(Default::default);

struct Mermaid;

impl MarkdownPlugin for Mermaid {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "mdk-mermaid"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Code(code) = node else {
            return None;
        };
        (code.lang.as_deref() == Some("mermaid"))
            .then(|| MarkdownNode::new(self.name(), code.value.clone()))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let source = node.data::<String>().cloned().unwrap_or_default();
        // The spike renders on the UI thread on first sight. Production does this in the background.
        let diagram = DIAGRAMS
            .lock()
            .unwrap()
            .entry(source.clone())
            .or_insert_with(|| {
                let options = mermaid_rs_renderer::RenderOptions {
                    theme: if cx.theme().is_dark() {
                        mermaid_rs_renderer::Theme::dark()
                    } else {
                        mermaid_rs_renderer::Theme::modern()
                    },
                    ..Default::default()
                };
                mermaid_rs_renderer::render_with_options(&source, options)
                    .map(|svg| Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
                    .map_err(|e| e.to_string())
            })
            .clone();
        match diagram {
            Ok(image) => {
                seen("mermaid");
                div().my_3().child(img(image).max_w_full())
            }
            Err(error) => {
                seen("mermaid-error");
                div()
                    .my_3()
                    .child(source)
                    .child(div().text_color(rgb(0xef4444)).child(error))
            }
        }
    }
}

// ---- callouts -------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Callout {
    pub kind: String,
    pub title: String,
    /// `Some(true)` for `-`, `Some(false)` for `+`, `None` when it cannot fold.
    pub folded: Option<bool>,
    pub body: String,
}

pub fn parse_callout(source: &str) -> Option<Callout> {
    let mut lines = source.lines().map(|l| {
        let l = l.trim_start();
        l.strip_prefix("> ").or(l.strip_prefix('>')).unwrap_or(l)
    });
    let (kind, after) = lines.next()?.strip_prefix("[!")?.split_once(']')?;
    let (folded, title) = match after.chars().next() {
        Some('-') => (Some(true), &after[1..]),
        Some('+') => (Some(false), &after[1..]),
        _ => (None, after),
    };
    let kind = kind.to_lowercase();
    let title = match title.trim() {
        "" => kind[..1].to_uppercase() + &kind[1..],
        t => t.to_string(),
    };
    Some(Callout {
        kind,
        title,
        folded,
        body: lines.collect::<Vec<_>>().join("\n"),
    })
}

struct CalloutPlugin;

impl MarkdownPlugin for CalloutPlugin {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "mdk-callout"
    }

    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Blockquote(_) = node else {
            return None;
        };
        let callout = parse_callout(cx.node_source(node)?)?;
        Some(MarkdownNode::new(self.name(), callout))
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let callout = node.data::<Callout>().cloned().unwrap();
        let key = node.source_range().map_or(0, |r| r.start);
        let color: Hsla = rgb(match callout.kind.as_str() {
            "tip" | "success" | "hint" => 0x22c55e,
            "important" | "faq" | "question" => 0x8b5cf6,
            "warning" | "attention" => 0xf59e0b,
            "caution" | "danger" | "error" | "bug" => 0xef4444,
            _ => 0x3b82f6,
        })
        .into();
        let start_open = !callout.folded.unwrap_or(false);
        let open = window.use_keyed_state(("callout-open", key), cx, |_, _| start_open);
        let is_open = *open.read(cx);
        seen(if is_open {
            "callout-open"
        } else {
            "callout-folded"
        });

        div()
            .my_3()
            .p_3()
            .rounded_md()
            .border_l_4()
            .border_color(color)
            .bg(color.opacity(0.12))
            .child(
                div()
                    .id(("callout", key))
                    .flex()
                    .gap_2()
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child(callout.title.clone())
                    .when(callout.folded.is_some(), |d| {
                        d.child(if is_open { "▾" } else { "▸" })
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                open.update(cx, |open, cx| {
                                    *open = !*open;
                                    cx.notify();
                                })
                            })
                    }),
            )
            .when(is_open, |d| {
                d.child(TextView::markdown(("callout-body", key), callout.body))
            })
    }
}

// ---- images and missing wikilinks -----------------------------------------------------------

#[derive(Clone)]
struct Picture {
    alt: String,
    url: String,
    width: Option<f32>,
}

struct Images {
    root: PathBuf,
}

impl MarkdownPlugin for Images {
    fn name(&self) -> &str {
        "mdk-image"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Image(image) = node else {
            return None;
        };
        let picture = Picture {
            alt: image.alt.clone(),
            url: image.url.replace("%20", " "),
            width: image.title.as_deref().and_then(|t| t.parse().ok()),
        };
        Some(MarkdownNode::new(self.name(), picture))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let picture = node.data::<Picture>().cloned().unwrap();
        let path = self.root.join(&picture.url);
        if picture.url.contains("://") {
            // Never fetched (NFR-02). Shown as the link it is.
            seen("image-remote");
            div()
                .text_color(cx.theme().link)
                .child(format!("{} ({})", picture.alt, picture.url))
                .into_any_element()
        } else if path.is_file() {
            seen("image-local");
            img(path)
                .w(px(picture.width.unwrap_or(320.)))
                .into_any_element()
        } else {
            seen("image-missing");
            div()
                .px_2()
                .border_1()
                .border_color(cx.theme().border)
                .child(picture.url)
                .into_any_element()
        }
    }
}

/// Links the source rewrite produced: `mdk-hl:` highlights and `mdk-missing:` wikilinks.
/// Each renders as one inline box, so a long highlight wraps as a unit.
struct Spans;

impl MarkdownPlugin for Spans {
    fn name(&self) -> &str {
        "mdk-span"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Link(link) = node else {
            return None;
        };
        let highlight = link.url == "mdk-hl:";
        if !highlight && !link.url.starts_with("mdk-missing:") {
            return None;
        }
        let label: String = link
            .children
            .iter()
            .filter_map(|child| match child {
                Node::Text(text) => Some(text.value.as_str()),
                _ => None,
            })
            .collect();
        Some(MarkdownNode::new(self.name(), highlight).text(label))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = node.as_text().to_string();
        if node.data::<bool>() == Some(&true) {
            seen("highlight");
            div()
                .px_0p5()
                .rounded_sm()
                .bg(rgb(0xfef08a))
                .text_color(rgb(0x1f2937))
                .child(label)
        } else {
            seen("wikilink-missing");
            div().text_color(cx.theme().muted_foreground).child(label)
        }
    }
}

// ---- the view -------------------------------------------------------------------------------

pub struct PreviewSpike {
    source: SharedString,
    root: PathBuf,
    /// The last `mdk-wiki:` target clicked.
    pub opened: Option<String>,
}

impl PreviewSpike {
    pub fn new(markdown: &str, root: PathBuf) -> Self {
        let source = preprocess(markdown, &|card| card == "other-card").into();
        Self {
            source,
            root,
            opened: None,
        }
    }
}

impl Render for PreviewSpike {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let view = TextView::markdown("card", self.source.clone())
            .selectable(true)
            .plugin(CalloutPlugin)
            .plugin(BlockMath)
            .plugin(Mermaid)
            .plugin(InlineMath)
            .plugin(Spans)
            .plugin(Images {
                root: self.root.clone(),
            })
            .code_block_actions(|block, _, cx| {
                seen("code-header");
                let code = block.code();
                div()
                    .flex()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(block.lang().unwrap_or_else(|| "text".into()))
                    .child(div().id("copy").cursor_pointer().child("Copy").on_click(
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()))
                        },
                    ))
            })
            .on_link_click(move |url, _, _, cx| match url.strip_prefix("mdk-wiki:") {
                Some(card) => this.update(cx, |this, cx| {
                    this.opened = Some(card.to_string());
                    cx.notify();
                }),
                // Only these schemes ever leave the app.
                None if url.starts_with("https://") || url.starts_with("http://") => {
                    cx.open_url(url)
                }
                None => {}
            });

        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(format!("opened: {:?}", self.opened))
            .child(view)
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: with test-support on, that glob shadows `#[test]` with GPUI's macro.
    use super::{dollar_guard, parse_callout, preprocess};

    #[test]
    fn rewrites_wikilinks_embeds_and_highlight_but_not_code_or_maths() {
        let out = preprocess(
            "a [[x|X]] [[gone]] ==hi== == no == ![[p q.png|96]] `[[x]] ==y==` $a==b==c$\n```\n[[x]]\n```\n",
            &|card| card == "x",
        );
        assert_eq!(
            out,
            "a [X](mdk-wiki:x) [gone](mdk-missing:gone) [hi](mdk-hl:) == no == \
             ![p q.png](p%20q.png \"96\") `[[x]] ==y==` $a==b==c$\n```\n[[x]]\n```\n"
        );
    }

    #[test]
    fn dollar_guard_keeps_prices_as_text() {
        assert!(dollar_guard("x^2", Some(' ')));
        assert!(!dollar_guard("5 a month, or ", Some('5')));
        assert!(!dollar_guard(" x", None));
        assert!(!dollar_guard("x ", None));
    }

    #[test]
    fn callout_headers() {
        let c = parse_callout("> [!NOTE]\n> body **b**").unwrap();
        assert_eq!(
            (c.kind.as_str(), c.title.as_str(), c.folded),
            ("note", "Note", None)
        );
        assert_eq!(c.body, "body **b**");
        let c = parse_callout("> [!faq]- Why?\n> because").unwrap();
        assert_eq!((c.title.as_str(), c.folded), ("Why?", Some(true)));
        assert_eq!(parse_callout("> [!tip]+ T").unwrap().folded, Some(false));
        assert_eq!(parse_callout("> just a quote"), None);
    }
}
