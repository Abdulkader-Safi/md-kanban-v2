//! The rendered preview: gpui-kit's `TextView` with mdk's plugins.
//!
//! Build one with [`view`]. It needs to know where the card is and which cards exist
//! ([`PreviewContext`]), and what to do when a link to a card is clicked.

use crate::source::{
    CARD_SCHEME, Callout, CardRef, HIGHLIGHT_SCHEME, MISSING_SCHEME, Tone, code_key, dollar_guard,
    find_link, is_external, is_image, parse_callout, prepare, resolve_path,
};
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

/// Where the card being previewed lives, and what it can link to.
#[derive(Clone)]
pub struct PreviewContext {
    /// The project folder. Images are read from inside it and nowhere else.
    pub root: PathBuf,
    /// The card's path in the project, for relative links and images.
    pub page_path: String,
    /// The project's cards, for wikilinks and links between cards.
    pub cards: Arc<Vec<CardRef>>,
}

type OpenCard = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync>;

const RED: u32 = 0xef4444;
/// How many rendered formulas and diagrams to keep.
const CACHE_SIZE: usize = 64;

/// Which kinds of custom element have been drawn. For tests.
static DRAWN: LazyLock<Mutex<BTreeSet<&'static str>>> = LazyLock::new(Default::default);

fn drew(kind: &'static str) {
    DRAWN.lock().unwrap().insert(kind);
}

/// The kinds of custom element drawn so far in this process: "math", "mermaid",
/// "callout-open" and so on.
pub fn drawn() -> BTreeSet<&'static str> {
    DRAWN.lock().unwrap().clone()
}

// ---- maths ----------------------------------------------------------------------------------

type Formula = Option<(Arc<RenderImage>, f32, f32)>;
static FORMULAS: LazyLock<Mutex<HashMap<String, Formula>>> = LazyLock::new(Default::default);

fn math(latex: &str, size: f32, window: &Window, cx: &App) -> AnyElement {
    let color = cx.theme().foreground;
    let scale = window.scale_factor();
    // The colour and size are part of the picture, so they are part of the key.
    let key = format!("{latex}\u{0}{size}\u{0}{scale}\u{0}{color:?}");
    let mut cache = FORMULAS.lock().unwrap();
    if cache.len() > CACHE_SIZE {
        cache.clear();
    }
    let formula = cache.entry(key).or_insert_with(|| {
        ratex_gpui::render::render_latex(latex, size, scale, color)
            .map(|r| (r.image, r.width, r.height))
    });
    match formula {
        Some((image, width, height)) => {
            drew("math");
            img(image.clone())
                .w(px(*width))
                .h(px(*height))
                .into_any_element()
        }
        None => {
            // A formula that does not parse shows its source, in red.
            drew("math-error");
            div()
                .text_color(rgb(RED))
                .child(format!("${latex}$"))
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
        let size = f32::from(context.font_size());
        Some(InlineElement::new(math(latex, size, window, cx)))
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

#[derive(Clone)]
enum Diagram {
    /// Being drawn on a background thread.
    Pending,
    Ready(Arc<Image>),
    Failed(String),
}

static DIAGRAMS: LazyLock<Mutex<HashMap<(String, bool), Diagram>>> =
    LazyLock::new(Default::default);

fn render_diagram(source: &str, dark: bool) -> Diagram {
    let options = mermaid_rs_renderer::RenderOptions {
        theme: if dark {
            mermaid_rs_renderer::Theme::dark()
        } else {
            mermaid_rs_renderer::Theme::modern()
        },
        ..Default::default()
    };
    // The renderer is young. A panic in it must not take the app down.
    let drawn =
        std::panic::catch_unwind(|| mermaid_rs_renderer::render_with_options(source, options));
    match drawn {
        Ok(Ok(svg)) => Diagram::Ready(Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            svg.into_bytes(),
        ))),
        Ok(Err(error)) => Diagram::Failed(error.to_string()),
        Err(_) => Diagram::Failed("The diagram renderer failed on this diagram".to_string()),
    }
}

/// The diagram for `source`, starting a background render the first time it is asked for.
fn diagram(source: &str, cx: &mut App) -> Diagram {
    let key = (source.to_string(), cx.theme().is_dark());
    let mut cache = DIAGRAMS.lock().unwrap();
    if let Some(diagram) = cache.get(&key) {
        return diagram.clone();
    }
    if cache.len() > CACHE_SIZE {
        cache.retain(|_, diagram| matches!(diagram, Diagram::Pending));
    }
    cache.insert(key.clone(), Diagram::Pending);
    drop(cache);
    cx.spawn(async move |cx| {
        let (source, dark) = key.clone();
        let done = cx
            .background_executor()
            .spawn(async move { render_diagram(&source, dark) })
            .await;
        DIAGRAMS.lock().unwrap().insert(key, done);
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
    Diagram::Pending
}

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
        let muted = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        match diagram(&source, cx) {
            Diagram::Ready(image) => {
                drew("mermaid");
                div().my_3().child(img(image).max_w_full())
            }
            Diagram::Pending => div().my_3().text_color(muted).child("Drawing diagram..."),
            Diagram::Failed(error) => {
                // A diagram that does not render shows its source and the error.
                drew("mermaid-error");
                div()
                    .my_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_family(mono).text_xs().child(source))
                    .child(div().text_xs().text_color(rgb(RED)).child(error))
            }
        }
    }
}

// ---- callouts -------------------------------------------------------------------------------

struct CalloutPlugin {
    context: PreviewContext,
    open_card: OpenCard,
}

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
        Some(MarkdownNode::new(
            self.name(),
            parse_callout(cx.node_source(node)?)?,
        ))
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let callout = node.data::<Callout>().cloned().expect("a callout node");
        let key = node.source_range().map_or(0, |range| range.start);
        let color: Hsla = match callout.tone {
            Tone::Note => rgb(0x3b82f6).into(),
            Tone::Tip => rgb(0x22c55e).into(),
            Tone::Warning => rgb(0xf59e0b).into(),
            Tone::Danger => rgb(RED).into(),
            Tone::Quote => cx.theme().muted_foreground,
        };
        let starts_open = !callout.folded.unwrap_or(false);
        let open = window.use_keyed_state(("callout-open", key), cx, |_, _| starts_open);
        let is_open = *open.read(cx);
        drew(if is_open {
            "callout-open"
        } else {
            "callout-folded"
        });

        div()
            .my_3()
            .p_3()
            .border_l_4()
            .border_color(color)
            .bg(color.opacity(0.1))
            .child(
                div()
                    .id(("callout", key))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child(callout.label.clone())
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
            .when(is_open && !callout.body.trim().is_empty(), |d| {
                // The body is markdown too, with the same plugins.
                d.child(div().mt_1().child(build(
                    ("callout-body", key),
                    &callout.body,
                    &self.context,
                    self.open_card.clone(),
                )))
            })
    }
}

// ---- images ---------------------------------------------------------------------------------

#[derive(Clone)]
struct Picture {
    alt: String,
    url: String,
    /// From `![[image.png|300]]` or `|300x200`.
    size: (Option<f32>, Option<f32>),
}

struct Images {
    context: PreviewContext,
}

impl MarkdownPlugin for Images {
    fn name(&self) -> &str {
        "mdk-image"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Image(image) = node else {
            return None;
        };
        let title = image.title.as_deref().unwrap_or("");
        let (width, height) = title.split_once('x').unwrap_or((title, ""));
        let picture = Picture {
            alt: image.alt.clone(),
            url: image.url.clone(),
            size: (width.trim().parse().ok(), height.trim().parse().ok()),
        };
        Some(MarkdownNode::new(self.name(), picture).text(image.alt.clone()))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let picture = node.data::<Picture>().cloned().expect("an image node");
        let name = picture
            .url
            .rsplit('/')
            .next()
            .unwrap_or(&picture.url)
            .replace("%20", " ");
        if picture.url.contains("://") {
            // Never fetched: mdk makes no network requests. Shown as the link it is.
            drew("image-remote");
            let label = if picture.alt.is_empty() {
                name
            } else {
                picture.alt
            };
            return div()
                .text_color(cx.theme().link)
                .child(format!("{label} ({})", picture.url))
                .into_any_element();
        }
        // Only image files, and only inside the project folder.
        let file = resolve_path(&self.context.page_path, &picture.url)
            .filter(|path| is_image(path))
            .map(|path| self.context.root.join(path))
            .filter(|file| file.is_file());
        match file {
            Some(file) => {
                drew("image-local");
                img(file)
                    .max_w_full()
                    .when_some(picture.size.0, |i, w| i.w(px(w)))
                    .when_some(picture.size.1, |i, h| i.h(px(h)))
                    .into_any_element()
            }
            None => {
                drew("image-missing");
                div()
                    .px_2()
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Image not found: {name}"))
                    .into_any_element()
            }
        }
    }
}

// ---- highlights and links that lead nowhere -------------------------------------------------

/// How a rewritten or dead link is drawn.
#[derive(Clone, Copy, PartialEq)]
enum Span {
    Highlight,
    /// A wikilink or a relative link with no card behind it.
    Missing,
}

struct Spans {
    context: PreviewContext,
}

impl MarkdownPlugin for Spans {
    fn name(&self) -> &str {
        "mdk-span"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Link(link) = node else {
            return None;
        };
        let url = link.url.as_str();
        let span = if url == HIGHLIGHT_SCHEME {
            Span::Highlight
        } else if url.starts_with(MISSING_SCHEME) {
            Span::Missing
        } else if url.starts_with(CARD_SCHEME) || url.starts_with('#') || is_external(url) {
            return None;
        } else if find_link(&self.context.cards, &self.context.page_path, url).is_some() {
            // A relative link to a card. It stays a link, and the click opens the card.
            return None;
        } else {
            // Anything else, such as a path out of the project or `javascript:`.
            Span::Missing
        };
        let label: String = link
            .children
            .iter()
            .filter_map(|child| match child {
                Node::Text(text) => Some(text.value.as_str()),
                Node::InlineCode(code) => Some(code.value.as_str()),
                _ => None,
            })
            .collect();
        Some(MarkdownNode::new(self.name(), span).text(label))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = node.as_text().to_string();
        if node.data::<Span>() == Some(&Span::Highlight) {
            drew("highlight");
            div()
                .px_0p5()
                .bg(rgb(0xfef08a))
                .text_color(rgb(0x1f2937))
                .child(label)
        } else {
            drew("link-missing");
            div().text_color(cx.theme().muted_foreground).child(label)
        }
    }
}

// ---- the view -------------------------------------------------------------------------------

fn build(
    id: impl Into<ElementId>,
    markdown: &str,
    context: &PreviewContext,
    open_card: OpenCard,
) -> TextView {
    let prepared = prepare(markdown, &context.cards);
    let titles = prepared.code_titles;
    let (link_context, link_open) = (context.clone(), open_card.clone());

    TextView::markdown(id, prepared.source)
        .selectable(true)
        .plugin(CalloutPlugin {
            context: context.clone(),
            open_card,
        })
        .plugin(BlockMath)
        .plugin(Mermaid)
        .plugin(InlineMath)
        .plugin(Spans {
            context: context.clone(),
        })
        .plugin(Images {
            context: context.clone(),
        })
        .code_block_actions(move |block, _, cx| {
            drew("code-header");
            let code = block.code();
            // The fence's title, or its language.
            let label = titles
                .get(&code_key(&code))
                .cloned()
                .or_else(|| block.lang().map(|lang| lang.to_string()))
                .unwrap_or_else(|| "text".to_string());
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .child(
                    div()
                        .id("copy-code")
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child("Copy")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()))
                        }),
                )
        })
        .on_link_click(move |url, _, window, cx| {
            if let Some(path) = url.strip_prefix(CARD_SCHEME) {
                link_open(
                    &path
                        .replace("%20", " ")
                        .replace("%28", "(")
                        .replace("%29", ")")
                        .replace("%25", "%"),
                    window,
                    cx,
                );
            } else if is_external(url) {
                // The only links that leave the app, and only through the OS.
                cx.open_url(url);
            } else if let Some(card) = find_link(&link_context.cards, &link_context.page_path, url)
            {
                link_open(&card.rel_path, window, cx);
            }
        })
}

/// The preview of a card body. `open_card` is called with a card's path in the project
/// when a wikilink or a link to that card is clicked.
pub fn view(
    id: impl Into<ElementId>,
    markdown: &str,
    context: &PreviewContext,
    open_card: impl Fn(&str, &mut Window, &mut App) + Send + Sync + 'static,
) -> TextView {
    build(id, markdown, context, Arc::new(open_card))
}
