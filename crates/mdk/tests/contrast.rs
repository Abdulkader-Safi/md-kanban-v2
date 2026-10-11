//! NFR-06: text contrast at least 4.5:1 and focus indicators at least 3:1, in both themes.
//! The colours are read from the theme file the app ships and from the code that picks
//! badge colours, so this fails if either drifts.

use gpui_kit::Rgba;
use mdk::theme::{due_colors, priority_colors};
use mdk_format::Priority;
use serde_json::Value;

type Rgb = [f64; 4];

fn hex(color: &str) -> Rgb {
    let h = color.trim_start_matches('#');
    let byte = |i: usize| f64::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap()) / 255.;
    [
        byte(0),
        byte(2),
        byte(4),
        if h.len() == 8 { byte(6) } else { 1. },
    ]
}

fn from_u32(color: u32) -> Rgb {
    hex(&format!("{color:06x}"))
}

fn from_gpui(color: impl Into<Rgba>) -> Rgb {
    let c: Rgba = color.into();
    [c.r.into(), c.g.into(), c.b.into(), c.a.into()]
}

/// Draw `top` over an opaque `under`.
fn over(top: Rgb, under: Rgb) -> Rgb {
    let a = top[3];
    [0, 1, 2, 3].map(|i| {
        if i == 3 {
            1.
        } else {
            top[i] * a + under[i] * (1. - a)
        }
    })
}

fn with_alpha(color: Rgb, alpha: f64) -> Rgb {
    [color[0], color[1], color[2], alpha]
}

/// WCAG 2 relative luminance and contrast ratio.
fn ratio(a: Rgb, b: Rgb) -> f64 {
    let luminance = |c: Rgb| {
        let lin = |v: f64| {
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    };
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn both_themes_meet_the_contrast_rules() {
    let themes: Value = serde_json::from_str(include_str!("../assets/theme/mdk.json")).unwrap();
    let mut failures = Vec::new();
    for theme in themes["themes"].as_array().unwrap() {
        let name = theme["name"].as_str().unwrap();
        let dark = theme["mode"] == "dark";
        let color = |key: &str| {
            hex(theme["colors"][key]
                .as_str()
                .unwrap_or_else(|| panic!("{name} has no {key}")))
        };
        let bg = color("background");
        let card = over(color("popover.background"), bg);
        let sidebar = over(color("sidebar.background"), bg);
        let muted = over(color("muted.background"), bg);
        let secondary = over(color("secondary.background"), bg);
        // Columns are the muted colour at 40% over the page.
        let column = over(with_alpha(muted, 0.4), bg);
        let muted_fg = color("muted.foreground");

        let mut check = |what: &str, fg: Rgb, on: Rgb, least: f64| {
            let got = ratio(over(fg, on), on);
            if got < least {
                failures.push(format!("{name}: {what} is {got:.2}:1, needs {least}:1"));
            }
        };

        // Text.
        check("text on the page", color("foreground"), bg, 4.5);
        check("text on a card", color("popover.foreground"), card, 4.5);
        check(
            "text in the sidebar",
            color("sidebar.foreground"),
            sidebar,
            4.5,
        );
        check("muted text on the page", muted_fg, bg, 4.5);
        check("muted text on a card", muted_fg, card, 4.5);
        check("muted text in a column", muted_fg, column, 4.5);
        check("muted text in the sidebar", muted_fg, sidebar, 4.5);
        check(
            "primary button text",
            color("primary.foreground"),
            color("primary.background"),
            4.5,
        );
        check(
            "selected row text",
            color("sidebar.accent.foreground"),
            color("sidebar.accent.background"),
            4.5,
        );
        check("chip text", color("secondary.foreground"), secondary, 4.5);
        check("muted chip text", muted_fg, secondary, 4.5);
        check("link text on the page", color("link"), bg, 4.5);
        check("link text on a card", color("link"), card, 4.5);
        check(
            "error text in the sidebar",
            color("danger.background"),
            sidebar,
            4.5,
        );
        for priority in Priority::ALL {
            let (text, tint) = priority_colors(priority, dark);
            let badge = over(from_gpui(tint), card);
            check(
                &format!("{} badge", priority.as_str()),
                from_gpui(text),
                badge,
                4.5,
            );
        }
        let (red, amber) = due_colors(dark);
        for (what, text) in [("overdue chip", red), ("due-today chip", amber)] {
            let chip = over(with_alpha(from_u32(text), 0.15), card);
            check(what, from_u32(text), chip, 4.5);
        }

        // Focus and drop indicators, which are drawn in the link colour.
        check("the focus ring on a card", color("link"), card, 3.0);
        check(
            "the focus ring against a column",
            color("link"),
            column,
            3.0,
        );
        check("the drop slot in a column", color("link"), column, 3.0);
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
