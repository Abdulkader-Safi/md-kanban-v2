use gpui_kit::*;
use mdk::app::Mdk;
use mdk::prefs::{Prefs, config_dir};

fn main() {
    let dir = config_dir();
    mdk::logging::init(&dir);
    let Some(instance) = mdk::app::single_instance(&dir) else {
        // Another mdk is running and has been asked to come to the front.
        return;
    };
    gpui_kit::application()
        .with_assets(mdk::theme::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            mdk::theme::init(cx);
            mdk::app::bind_keys(cx);
            let (prefs, notice) = Prefs::load(&dir);
            let watch = std::env::var_os("MDK_NO_WATCH").is_none_or(|v| v != "1");
            let options = mdk::app::window_options(&prefs, cx);
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Mdk::new(prefs, dir, notice, instance, watch, window, cx))
            })
            .expect("the window opens");
            cx.activate(true);
        });
}
