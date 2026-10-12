//! The command palette and the settings dialog.

use super::{Colors, dialog, dialog_footer, dialog_header, icon_button, kbd, toggle};
use crate::app::Mdk;
use crate::board::PaletteItem;
use crate::prefs::ThemeChoice;
use crate::theme::icon;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, MoveDown, MoveUp};
use gpui_kit::prelude::*;
use gpui_kit::*;

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
            let icon_name = match item {
                PaletteItem::Card(_) => "layout-kanban",
                PaletteItem::Create(_) => "plus",
                PaletteItem::Move(_) => "list",
                PaletteItem::Command(_) => "source-code",
            };
            let item = item.clone();
            let on = n == selected;
            div()
                .id(("palette-item", n))
                .flex()
                .items_center()
                .gap_3()
                .px_4()
                .py_2()
                .border_l_2()
                .cursor_pointer()
                .map(|d| match on {
                    true => d.bg(c.tint).border_color(c.primary),
                    false => d
                        .border_color(c.primary.opacity(0.))
                        .hover(|s| s.bg(c.secondary)),
                })
                .child(icon(icon_name).size_4().text_color(match on {
                    true => c.link,
                    false => c.muted_fg,
                }))
                .child(div().flex_1().truncate().child(label))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(px(11.))
                        .text_color(c.muted_fg)
                        .child(hint),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.run_palette_item(item.clone(), window, cx)
                }))
        });
        let rows: Vec<_> = rows.collect();
        let empty = rows.is_empty();
        let count = rows.len();

        let (backdrop, panel) = dialog(
            "palette",
            640.,
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
                            .flex()
                            .items_center()
                            .gap_3()
                            .h(px(52.))
                            .px_4()
                            .border_b_2()
                            .border_color(c.primary)
                            .child(icon("search").size_4().text_color(c.muted_fg))
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&palette.input).appearance(false)),
                            )
                            .child(kbd("esc", c, cx)),
                    )
                    .child(
                        div()
                            .id("palette-items")
                            .max_h(px(360.))
                            .overflow_y_scroll()
                            .children(rows)
                            .when(empty, |d| {
                                d.child(
                                    div()
                                        .px_4()
                                        .py_3()
                                        .text_color(c.muted_fg)
                                        .child("No matches"),
                                )
                            }),
                    )
                    .child(
                        dialog_footer(c)
                            .py_2()
                            .text_xs()
                            .text_color(c.muted_fg)
                            .child(kbd("↑↓", c, cx))
                            .child("move")
                            .child(kbd("↵", c, cx))
                            .child("run")
                            .child(kbd("esc", c, cx))
                            .child("close")
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(mono)
                                    .text_size(px(10.))
                                    .child(format!("{count} RESULTS")),
                            ),
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
        let row = |title: &'static str, about: &'static str, control: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap_4()
                .px_4()
                .py_3p5()
                .border_b_1()
                .border_color(c.border)
                .child(
                    div()
                        .flex_1()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(div().text_xs().text_color(c.muted_fg).child(about)),
                )
                .child(control)
        };
        let (backdrop, panel) = dialog("settings", 600., c, cx.listener(close));
        backdrop.items_center().child(
            panel
                .child(
                    dialog_header("Settings", c)
                        .child(icon_button("settings-x", "x", c).on_click(cx.listener(close))),
                )
                .child(row(
                    "Theme",
                    "Dark, light, or follow the system.",
                    super::choice(
                        "settings-theme",
                        themes,
                        theme,
                        c,
                        cx,
                        |this, choice, window, cx| this.set_theme(choice, window, cx),
                    )
                    .into_any_element(),
                ))
                .child(row(
                    "Auto-refresh folders",
                    "Reload cards when files change on disk.",
                    toggle("settings-auto-refresh", self.prefs.auto_refresh, c)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_auto_refresh(!this.prefs.auto_refresh, cx)
                        }))
                        .into_any_element(),
                ))
                .child(row(
                    "Vim keys in editor",
                    "Normal, insert and visual modes in the card editor.",
                    toggle("settings-vim", self.prefs.vim_keys, c)
                        .on_click(
                            cx.listener(|this, _, _, cx| {
                                this.set_vim_keys(!this.prefs.vim_keys, cx)
                            }),
                        )
                        .into_any_element(),
                ))
                .child(
                    dialog_footer(c)
                        .child(
                            Button::new("open-config")
                                .outline()
                                .label("Open config folder")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.open_config_folder(cx)),
                                ),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("settings-close")
                                .primary()
                                .label("Done")
                                .on_click(cx.listener(close)),
                        ),
                ),
        )
    }
}
