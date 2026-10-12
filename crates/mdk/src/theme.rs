//! The mdk theme: ink surfaces and one lime accent as a gpui-kit theme, plus the bundled
//! Schibsted Grotesk and JetBrains Mono fonts and Tabler icons.

use crate::prefs::ThemeChoice;
use gpui_kit::component::{Icon, Theme, ThemeMode, ThemeSet};
use gpui_kit::{App, AssetSource, Hsla, Result, SharedString, Window, rgb};
use mdk_format::{Priority, Status};
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
        .add_fonts(
            [
                include_bytes!("../assets/fonts/SchibstedGrotesk-Regular.ttf").as_slice(),
                include_bytes!("../assets/fonts/SchibstedGrotesk-Medium.ttf").as_slice(),
                include_bytes!("../assets/fonts/SchibstedGrotesk-SemiBold.ttf").as_slice(),
                include_bytes!("../assets/fonts/SchibstedGrotesk-Bold.ttf").as_slice(),
                include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
                include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf").as_slice(),
                include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf").as_slice(),
            ]
            .map(Cow::Borrowed)
            .to_vec(),
        )
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

/// Badge colours for a priority: (text and outline, background). The badge is an
/// outline, so the background is clear.
pub fn priority_colors(priority: Priority, dark: bool) -> (Hsla, Hsla) {
    let text = match (priority, dark) {
        (Priority::Critical, true) => 0xff6b63,
        (Priority::High, true) => 0xffa24a,
        (Priority::Medium, true) => 0x6fa8ff,
        (Priority::Low, true) => 0x9b9ba3,
        (Priority::Critical, false) => 0xc2241c,
        (Priority::High, false) => 0xa84b00,
        (Priority::Medium, false) => 0x1f55c4,
        (Priority::Low, false) => 0x5f5f66,
    };
    let text: Hsla = rgb(text).into();
    (text, text.opacity(0.))
}

/// The column colour of a status.
pub fn status_color(status: Status, dark: bool) -> Hsla {
    rgb(match (status, dark) {
        (Status::Backlog, true) => 0x8a8a93,
        (Status::Todo, true) => 0x6fa8ff,
        (Status::InProgress, true) => 0xffb020,
        (Status::Review, true) => 0xa78bfa,
        (Status::Done, true) => 0x3ddc84,
        (Status::Backlog, false) => 0x6b6b74,
        (Status::Todo, false) => 0x1f55c4,
        (Status::InProgress, false) => 0xa84b00,
        (Status::Review, false) => 0x6d45d9,
        (Status::Done, false) => 0x16803f,
    })
    .into()
}

/// The overdue and due-today colours.
pub fn due_colors(dark: bool) -> (u32, u32) {
    if dark {
        (0xff6b63, 0xffb020)
    } else {
        (0xc2241c, 0xa84b00)
    }
}
