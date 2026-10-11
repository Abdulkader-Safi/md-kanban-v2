//! The command palette and the settings dialog.

use super::Colors;
use crate::app::Mdk;
use crate::board::PaletteItem;
use crate::prefs::ThemeChoice;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, MoveDown, MoveUp};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

/// A centred dialog over a dark backdrop. A click on the backdrop runs `close`.
fn dialog(
    id: &'static str,
    width: f32,
    c: Colors,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> (Stateful<Div>, Stateful<Div>) {
    let backdrop = div()
        .id(SharedString::from(format!("{id}-backdrop")))
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .bg(black().opacity(0.4))
        .on_click(close);
    let panel = div()
        .id(id)
        .occlude()
        .w(px(width))
        .max_w_full()
        .flex()
        .flex_col()
        .bg(c.bg)
        .border_1()
        .border_color(c.border)
        .shadow_lg()
        .on_click(|_, _, cx| cx.stop_propagation());
    (backdrop, panel)
}

impl Mdk {
    pub(super) fn palette_view(
        &self,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let Some(palette) = &self.palette else {
            return div().into_any_element();
        };
        let mono = cx.theme().mono_font_family.clone();
        let items = self.palette_items(cx);
        let selected = palette.selected.min(items.len().saturating_sub(1));

        let rows = items.iter().enumerate().map(|(n, item)| {
            let (label, hint) = match item {
                PaletteItem::Card(key) => match self.board.card(key) {
                    Some(card) => (card.data.title.clone(), card.data.status.name().to_string()),
                    None => (key.rel_path.clone(), String::new()),
                },
                PaletteItem::Create(text) => (format!("Create card \"{text}\""), "new".into()),
                PaletteItem::Move(status) => (
                    format!("Move open card to {}", status.name()),
                    "move".into(),
                ),
                PaletteItem::Command(command) => (command.label().to_string(), "command".into()),
            };
            let item = item.clone();
            div()
                .id(("palette-item", n))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1p5()
                .cursor_pointer()
                .map(|d| match n == selected {
                    true => d.bg(c.accent).text_color(c.accent_fg),
                    false => d.hover(|s| s.bg(c.secondary)),
                })
                .child(div().flex_1().truncate().child(label))
                .child(div().text_size(px(11.)).opacity(0.7).child(hint))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.run_palette_item(item.clone(), window, cx)
                }))
        });
        let rows: Vec<_> = rows.collect();
        let empty = rows.is_empty();

        let (backdrop, panel) = dialog(
            "palette",
            560.,
            c,
            cx.listener(|this, _, window, cx| this.toggle_palette(window, cx)),
        );
        backdrop
            .items_start()
            .pt_20()
            .child(
                panel
                    // Up and Down belong to the list, not to the text box.
                    .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                        this.palette_step(false, cx);
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                        this.palette_step(true, cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        div()
                            .p_2()
                            .child(Input::new(&palette.input).appearance(false)),
                    )
                    .child(
                        div()
                            .id("palette-items")
                            .max_h(px(360.))
                            .overflow_y_scroll()
                            .border_t_1()
                            .border_color(c.border)
                            .children(rows)
                            .when(empty, |d| {
                                d.child(
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_color(c.muted_fg)
                                        .child("No matches"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1p5()
                            .border_t_1()
                            .border_color(c.border)
                            .font_family(mono)
                            .text_size(px(10.))
                            .text_color(c.muted_fg)
                            .child("↑↓ navigate · Enter run · Esc close"),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn settings_dialog(
        &self,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = self.prefs.theme;
        let themes = [
            (ThemeChoice::Dark, "Dark"),
            (ThemeChoice::Light, "Light"),
            (ThemeChoice::System, "System"),
        ];
        let close = |this: &mut Mdk, _: &ClickEvent, window: &mut Window, cx: &mut Context<Mdk>| {
            this.settings_open = false;
            this.focus.focus(window, cx);
            cx.notify();
        };
        let (backdrop, panel) = dialog("settings", 384., c, cx.listener(close));
        backdrop.items_center().child(
            panel
                .gap_3()
                .p_4()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .child("Settings"),
                )
                .child(super::field("Theme", c).child(super::choice(
                    "settings-theme",
                    themes,
                    theme,
                    c,
                    cx,
                    |this, choice, window, cx| this.set_theme(choice, window, cx),
                )))
                .child(
                    Switch::new("settings-auto-refresh")
                        .small()
                        .checked(self.prefs.auto_refresh)
                        .label("Auto-refresh folders")
                        .on_click(
                            cx.listener(|this, on: &bool, _, cx| this.set_auto_refresh(*on, cx)),
                        ),
                )
                .child(
                    Switch::new("settings-vim")
                        .small()
                        .checked(self.prefs.vim_keys)
                        .label("Vim keys in editor")
                        .on_click(cx.listener(|this, on: &bool, _, cx| this.set_vim_keys(*on, cx))),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .pt_1()
                        .child(
                            Button::new("open-config")
                                .outline()
                                .small()
                                .label("Open config folder")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.open_config_folder(cx)),
                                ),
                        )
                        .child(
                            Button::new("settings-close")
                                .ghost()
                                .small()
                                .label("Close")
                                .on_click(cx.listener(close)),
                        ),
                ),
        )
    }
}
