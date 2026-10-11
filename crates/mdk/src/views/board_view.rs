//! The board: five columns of card faces.

use super::Colors;
use crate::app::{Mdk, today};
use crate::board::Card;
use crate::theme::{AMBER, RED, icon, priority_colors};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::{Date, Status, Tone, format_due};

const COLUMN_WIDTH: Pixels = px(288.);

fn chip(c: Colors) -> Div {
    div()
        .px_2()
        .py_0p5()
        .bg(c.secondary)
        .font_weight(FontWeight::MEDIUM)
}

/// The face of one card, as Electron's `TaskCard.vue` lays it out.
fn card_face(card: &Card, today: Date, mono: SharedString, c: Colors) -> Stateful<Div> {
    let data = &card.data;
    let (priority_fg, priority_bg) = priority_colors(data.priority);
    // CSS collapses line breaks in the excerpt. Do the same.
    let excerpt = card
        .excerpt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let due = data.due_date.as_ref().map(|due| {
        let (label, tone) = format_due(due, today);
        let chip = match tone {
            Tone::Red => chip(c).bg(rgb(RED).opacity(0.15)).text_color(rgb(RED)),
            Tone::Amber => chip(c).bg(rgb(AMBER).opacity(0.15)).text_color(rgb(AMBER)),
            Tone::Muted => chip(c).text_color(c.fg),
        };
        chip.child(format!("{label} · {due}"))
    });
    let (done, total) = card.subtasks;
    let has_chips =
        !data.assignee.is_empty() || due.is_some() || !card.workspace.is_empty() || total > 0;
    let extra_labels = data.labels.len().saturating_sub(3);

    div()
        .id(SharedString::from(format!(
            "card-{}-{}",
            card.key.project, card.key.rel_path
        )))
        .flex()
        .flex_col()
        .p_3()
        .border_1()
        .border_color(c.border)
        .bg(c.card)
        .shadow_sm()
        .cursor_pointer()
        .hover(|s| s.border_color(c.primary.opacity(0.6)))
        .child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(20.))
                        .child(data.title.clone()),
                )
                .when_some(card.error().map(str::to_string), |d, error| {
                    d.child(
                        div()
                            .id("card-error")
                            .child(icon("alert-triangle").size_4().text_color(rgb(AMBER)))
                            .tooltip(move |window, cx| {
                                Tooltip::new(format!("Frontmatter error: {error}"))
                                    .build(window, cx)
                            }),
                    )
                })
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(priority_bg)
                        .text_color(priority_fg)
                        .child(data.priority.as_str()),
                )
                // The move menu arrives in M3.
                .child(icon("dots").size_4().text_color(c.muted_fg)),
        )
        .when(!excerpt.is_empty(), |d| {
            d.child(
                div()
                    .mt_1()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(c.muted_fg)
                    .line_clamp(2)
                    .child(excerpt),
            )
        })
        .when(has_chips, |d| {
            d.child(
                div()
                    .mt_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1p5()
                    .text_size(px(11.))
                    .text_color(c.muted_fg)
                    .when(!data.assignee.is_empty(), |d| {
                        d.child(chip(c).child(format!("@{}", data.assignee)))
                    })
                    .children(due)
                    .when(!card.workspace.is_empty(), |d| {
                        d.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .bg(c.muted)
                                .font_family(mono.clone())
                                .child(card.workspace.clone()),
                        )
                    })
                    .when(total > 0, |d| {
                        d.child(chip(c).child(format!("☑ {done}/{total}")))
                    }),
            )
        })
        .when(!data.labels.is_empty(), |d| {
            d.child(
                div()
                    .mt_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.))
                    .text_color(c.muted_fg)
                    .children(data.labels.iter().take(3).map(|label| {
                        div()
                            .px_1p5()
                            .border_1()
                            .border_color(c.border)
                            .child(format!("#{label}"))
                    }))
                    .when(extra_labels > 0, |d| {
                        d.child(format!("+{extra_labels} more"))
                    }),
            )
        })
        .child(
            div()
                .mt_1p5()
                .font_family(mono)
                .text_size(px(10.))
                .text_color(c.muted_fg.opacity(0.7))
                .child(card.key.rel_path.clone()),
        )
}

impl Mdk {
    pub(super) fn board_view(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let columns: Vec<_> = Status::ALL
            .into_iter()
            .enumerate()
            .map(|(i, status)| self.column(i, status, c, cx))
            .collect();
        // The board scrolls sideways only. Each column scrolls its own cards.
        div()
            .id("board")
            .flex()
            .flex_1()
            .min_h_0()
            .gap_3()
            .pb_2()
            .overflow_x_scroll()
            .children(columns)
    }

    fn column(
        &self,
        i: usize,
        status: Status,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let count = self.columns[i].len();
        let this = cx.entity();
        let mono = cx.theme().mono_font_family.clone();
        let today = today();

        // One row per card, then a last row holding "+ Add card".
        let cards = list(self.lists[i].clone(), move |row, _, cx| {
            let view = this.read(cx);
            let Some((key, _)) = view.columns[i].get(row) else {
                let this = this.clone();
                return div()
                    .id(("add-card", i))
                    .px_2()
                    .pb_3()
                    .child(
                        div()
                            .p_2()
                            .border_1()
                            .border_dashed()
                            .border_color(c.border)
                            .text_xs()
                            .text_color(c.muted_fg)
                            .text_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                            .child("+ Add card"),
                    )
                    .on_click(move |_, _, cx| this.update(cx, |this, cx| this.new_card(status, cx)))
                    .into_any_element();
            };
            let Some(card) = view.board.card(key) else {
                return div().into_any_element();
            };
            let (key, this) = (key.clone(), this.clone());
            div()
                .px_2()
                .pb_2()
                .child(
                    card_face(card, today, mono.clone(), c).on_click(move |_, _, cx| {
                        this.update(cx, |this, cx| this.open(key.clone(), cx))
                    }),
                )
                .into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(COLUMN_WIDTH)
            .h_full()
            .border_1()
            .border_color(c.border)
            .bg(c.muted.opacity(0.4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .child(div().size_2p5().rounded_full().bg(rgb(status.color())))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(status.name()))
                    .child(div().flex_1())
                    .child(
                        div()
                            .px_2()
                            .bg(c.secondary)
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child(count.to_string()),
                    )
                    .child(
                        div()
                            .id(("column-add", i))
                            .px_1p5()
                            .text_lg()
                            .line_height(px(18.))
                            .text_color(c.muted_fg)
                            .cursor_pointer()
                            .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                            .child("+")
                            .on_click(cx.listener(move |this, _, _, cx| this.new_card(status, cx))),
                    ),
            )
            .child(cards.flex_1().min_h_0())
    }
}
