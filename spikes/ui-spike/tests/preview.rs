use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use std::path::PathBuf;
use ui_spike::preview::{PreviewSpike, SEEN};

#[gpui_kit::test]
fn every_preview_element_renders_through_its_plugin(cx: &mut TestAppContext) {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));
    let window = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    // Tall, so nothing is scrolled out of the first layout.
                    size: size(px(1000.), px(3000.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| PreviewSpike::new(include_str!("../assets/preview-card.md"), root)),
        )
        .unwrap()
        .0
    });
    // Markdown parses on a background task, so draw, let it finish, and draw again.
    for _ in 0..3 {
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }

    let seen = SEEN.lock().unwrap().clone();
    for name in [
        "callout-open",
        "callout-folded",
        "math",
        "math-error",
        "mermaid",
        "code-header",
        "highlight",
        "wikilink-missing",
        "image-local",
        "image-remote",
    ] {
        assert!(seen.contains(name), "{name} did not render. Saw {seen:?}");
    }
    assert!(!seen.contains("mermaid-error"));
}
