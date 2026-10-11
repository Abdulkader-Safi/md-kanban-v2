//! The mdk theme: Electron MD Kanban's colours as a gpui-kit theme, plus the bundled
//! Geist font and Tabler icons.

use crate::prefs::ThemeChoice;
use gpui_kit::component::{Icon, Theme, ThemeMode, ThemeSet};
use gpui_kit::{App, AssetSource, Hsla, Result, SharedString, Window, rgb};
use mdk_format::Priority;
use std::borrow::Cow;
use std::rc::Rc;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn icon_bytes(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice()),)*
                _ => None,
            }
        }
    };
}

icons![
    "alert-triangle",
    "arrows-maximize",
    "arrows-minimize",
    "binary-tree",
    "blockquote",
    "bold",
    "chevron-down",
    "chevron-right",
    "code",
    "dots",
    "folder-open",
    "folders",
    "h-1",
    "h-2",
    "h-3",
    "highlight",
    "info-square-rounded",
    "italic",
    "layout-grid",
    "layout-kanban",
    "link",
    "list",
    "list-check",
    "list-numbers",
    "math-function",
    "moon",
    "photo",
    "plug-off",
    "plus",
    "refresh",
    "robot",
    "search",
    "separator-horizontal",
    "source-code",
    "strikethrough",
    "sun",
    "table",
    "trash",
    "x",
];

/// mdk's own icons under `mdk/`, and gpui-kit's assets for everything else.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path.strip_prefix("mdk/") {
            Some(name) => Ok(icon_bytes(name).map(Cow::Borrowed)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}

/// One of the bundled Tabler icons, by file name without `.svg`.
pub fn icon(name: &'static str) -> Icon {
    Icon::default().path(format!("mdk/{name}"))
}

/// Load the fonts and register the two mdk themes. Call once, after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-400.otf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-500.otf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-600.otf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-700.otf").as_slice()),
        ])
        .expect("the bundled fonts load");
    let set: ThemeSet = serde_json::from_str(include_str!("../assets/theme/mdk.json"))
        .expect("the bundled theme parses");
    Theme::update(cx, |theme| {
        for config in set.themes {
            if config.mode.is_dark() {
                theme.dark_theme = Rc::new(config);
            } else {
                theme.light_theme = Rc::new(config);
            }
        }
    });
}

/// Switch to the chosen theme. "System" follows the OS appearance.
pub fn apply(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    match choice {
        ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
    }
}

/// Badge colours for a priority: (text, background). The light theme uses darker
/// shades, so the text keeps a 4.5:1 contrast on the tinted badge.
pub fn priority_colors(priority: Priority, dark: bool) -> (Hsla, Hsla) {
    let text = match (priority, dark) {
        (Priority::Critical, true) => 0xf87171,
        (Priority::High, true) => 0xfb923c,
        (Priority::Medium, true) => 0x60a5fa,
        (Priority::Low, true) => 0x9ca3af,
        (Priority::Critical, false) => 0xb91c1c,
        (Priority::High, false) => 0x9a3412,
        (Priority::Medium, false) => 0x1d4ed8,
        (Priority::Low, false) => 0x4b5563,
    };
    let text: Hsla = rgb(text).into();
    (text, text.opacity(0.15))
}

/// The overdue and due-today colours, as text on a 15% tint of themselves.
pub fn due_colors(dark: bool) -> (u32, u32) {
    if dark {
        (0xf87171, 0xfbbf24)
    } else {
        (0xb91c1c, 0x92400e)
    }
}
