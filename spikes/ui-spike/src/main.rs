use gpui_kit::*;
use ui_spike::{dnd, vim};

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
            _ => {
                eprintln!("usage: ui-spike vim|dnd|preview");
                cx.quit();
            }
        }
    });
}
