use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use ui_spike::{dnd, preview, vim};

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let opts = WindowOptions::default();
        match which.as_str() {
            "vim" => {
                vim::bind_keys(cx);
                gpui_kit::open_window(opts, cx, |window, cx| {
                    cx.new(|cx| {
                        vim::VimSpike::new(
                            "# Title\n\nfirst line\nsecond line\nthird line\n",
                            window,
                            cx,
                        )
                    })
                })
                .unwrap();
            }
            "dnd" => {
                gpui_kit::open_window(opts, cx, |_, cx| cx.new(|_| dnd::DndSpike::new(200)))
                    .unwrap();
            }
            "preview" => {
                let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));
                gpui_kit::open_window(opts, cx, |window, cx| {
                    Theme::change(ThemeMode::Dark, Some(window), cx);
                    cx.new(|_| {
                        preview::PreviewSpike::new(include_str!("../assets/preview-card.md"), root)
                    })
                })
                .unwrap();
            }
            _ => {
                eprintln!("usage: ui-spike vim|dnd|preview");
                cx.quit();
            }
        }
    });
}
