//! The board: five columns of card faces.

use super::Colors;
use crate::app::{DragCard, Mdk, today};
use crate::board::{Card, CardKey};
use crate::theme::{due_colors, icon, priority_colors};
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

/// How a card is drawn beyond its own data.
#[derive(Clone, Copy, Default)]
struct CardState {
    /// The keyboard is on it.
    focused: bool,
    /// Picked up with Space.
    grabbed: bool,
    /// Being dragged with the mouse.
    dragging: bool,
    menu_open: bool,
}

/// The "⋯" menu: the five columns, with the card's own column disabled.
fn move_menu(key: &CardKey, current: Status, this: &Entity<Mdk>, c: Colors) -> impl IntoElement {
    let close = this.clone();
    div()
        .id("move-menu")
        .occlude()
        .mt_5()
        .w(px(144.))
        .p_1()
        .bg(c.card)
        .border_1()
        .border_color(c.border)
        .shadow_lg()
        .on_mouse_down_out(move |_, _, cx| {
            close.update(cx, |this, cx| {
                this.menu_for = None;
                cx.notify();
            })
        })
        .children(Status::ALL.into_iter().enumerate().map(|(n, status)| {
            let (this, key) = (this.clone(), key.clone());
            div()
                .id(("move-to", n))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1p5()
                .text_xs()
                .text_color(c.fg)
                .child(div().size_2().rounded_full().bg(rgb(status.color())))
                .child(status.name())
                .map(|d| match status == current {
                    true => d.opacity(0.4),
                    false => d
                        .cursor_pointer()
                        .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            this.update(cx, |this, cx| this.move_card(&key, status, None, cx))
                        }),
                })
        }))
}

/// The face of one card, as Electron's `TaskCard.vue` lays it out.
fn card_face(
    card: &Card,
    state: CardState,
    this: &Entity<Mdk>,
    today: Date,
    mono: SharedString,
    c: Colors,
) -> Stateful<Div> {
    let data = &card.data;
    let (priority_fg, priority_bg) = priority_colors(data.priority, c.dark);
    let (red, amber) = due_colors(c.dark);
    // CSS collapses line breaks in the excerpt. Do the same.
    let excerpt = card
        .excerpt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let due = data.due_date.as_ref().map(|due| {
        let (label, tone) = format_due(due, today);
        let chip = match tone {
            Tone::Red => chip(c).bg(rgb(red).opacity(0.15)).text_color(rgb(red)),
            Tone::Amber => chip(c).bg(rgb(amber).opacity(0.15)).text_color(rgb(amber)),
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
        .map(|d| match state.focused || state.grabbed {
            // The focus ring.
            true => d.border_color(c.link),
            false => d.hover(|s| s.border_color(c.primary.opacity(0.6))),
        })
        .when(state.dragging, |d| d.opacity(0.4))
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
                            .child(icon("alert-triangle").size_4().text_color(rgb(amber)))
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
                .child({
                    let (toggle, key) = (this.clone(), card.key.clone());
                    div()
                        .id("card-menu")
                        .px_0p5()
                        .cursor_pointer()
                        .text_color(c.muted_fg)
                        .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                        .child(icon("dots").size_4())
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            toggle.update(cx, |this, cx| {
                                let open = this.menu_for.as_ref() == Some(&key);
                                this.menu_for = (!open).then(|| key.clone());
                                cx.notify();
                            })
                        })
                        .when(state.menu_open, |d| {
                            d.child(
                                deferred(anchored().snap_to_window().child(move_menu(
                                    &card.key,
                                    data.status,
                                    this,
                                    c,
                                )))
                                .with_priority(1),
                            )
                        })
                }),
        )
        .when(!excerpt.is_empty(), |d| {
            d.child(
                div()
                    .mt_1()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(c.muted_fg)
                    .line_clamp(2)
                    .text_ellipsis()
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
                .text_color(c.muted_fg)
                .child(card.key.rel_path.clone()),
        )
        .when(state.grabbed, |d| {
            d.child(
                div()
                    .mt_2()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.link)
                    .child("Moving: arrows shift, Space drops, Esc cancels"),
            )
        })
}

/// The line that shows where a dragged card would land.
fn drop_slot(c: Colors) -> Div {
    div()
        .absolute()
        .top(px(-5.))
        .left_2()
        .right_2()
        .h(px(2.))
        .bg(c.link)
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
            .track_scroll(&self.board_scroll)
            .on_drag_move(
                cx.listener(|this, ev: &DragMoveEvent<DragCard>, _, cx| {
                    this.board_drag_move(ev, cx)
                }),
            )
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
            let slot = view.slot == Some((i, row));
            let Some((key, _)) = view.columns[i].get(row) else {
                let this = this.clone();
                return div()
                    .id(("add-card", i))
                    .relative()
                    .px_2()
                    .pb_3()
                    .when(slot, |d| d.child(drop_slot(c)))
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
                    .on_click(move |_, window, cx| {
                        this.update(cx, |this, cx| this.open_new_card(status, window, cx))
                    })
                    .into_any_element();
            };
            let Some(card) = view.board.card(key) else {
                return div().into_any_element();
            };
            let state = CardState {
                focused: view.focused.as_ref() == Some(key),
                grabbed: view.grabbed.as_ref().is_some_and(|g| g.key == *key),
                dragging: view.dragging.as_ref() == Some(key),
                menu_open: view.menu_for.as_ref() == Some(key),
            };
            let drag = DragCard {
                key: key.clone(),
                title: card.data.title.clone().into(),
            };
            let (open, started, key) = (this.clone(), this.clone(), key.clone());
            let dragged = key.clone();
            div()
                .relative()
                .px_2()
                .pb_2()
                .when(slot, |d| d.child(drop_slot(c)))
                .child(
                    card_face(card, state, &this, today, mono.clone(), c)
                        .on_click(move |_, window, cx| {
                            open.update(cx, |this, cx| this.open(key.clone(), window, cx))
                        })
                        .on_drag(drag, move |drag, _, _, cx| {
                            started.update(cx, |this, cx| {
                                this.dragging = Some(dragged.clone());
                                this.menu_for = None;
                                cx.notify();
                            });
                            cx.new(|_| drag.clone())
                        }),
                )
                .into_any_element()
        });

        let archivable = status == Status::Done && count > 0;
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(COLUMN_WIDTH)
            .h_full()
            .border_1()
            .border_color(c.border)
            .bg(c.muted.opacity(0.4))
            .drag_over::<DragCard>(move |style, _, _, _| style.bg(c.primary.opacity(0.15)))
            .on_drag_move(
                cx.listener(move |this, ev: &DragMoveEvent<DragCard>, _, cx| {
                    this.drag_move(i, ev, cx)
                }),
            )
            .on_drop(cx.listener(move |this, drag: &DragCard, _, cx| this.drop_card(i, drag, cx)))
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
                    .when(archivable, |d| {
                        d.child(
                            div()
                                .id("archive")
                                .px_1p5()
                                .py_0p5()
                                .text_size(px(11.))
                                .text_color(c.muted_fg)
                                .cursor_pointer()
                                .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                                .child("archive")
                                .on_click(cx.listener(|this, _, _, cx| this.ask_archive(cx))),
                        )
                    })
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_new_card(status, window, cx)
                            })),
                    ),
            )
            .child(cards.flex_1().min_h_0())
    }
}
