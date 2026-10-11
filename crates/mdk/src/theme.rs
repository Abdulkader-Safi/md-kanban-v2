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
    "dots",
    "folder-open",
    "folders",
    "layout-grid",
    "layout-kanban",
    "moon",
    "plug-off",
    "plus",
    "refresh",
    "search",
    "sun",
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

/// Badge colours for a priority: (text, background).
pub fn priority_colors(priority: Priority) -> (Hsla, Hsla) {
    let base: Hsla = rgb(match priority {
        Priority::Critical => 0xef4444,
        Priority::High => 0xf97316,
        Priority::Medium => 0x3b82f6,
        Priority::Low => 0x9ca3af,
    })
    .into();
    (base, base.opacity(0.15))
}

pub const RED: u32 = 0xef4444;
pub const AMBER: u32 = 0xf59e0b;
