//! The board: five columns of card faces.

use super::{Colors, icon_button, priority_badge, status_dot};
use crate::app::{DragCard, Mdk, today};
use crate::board::{Card, CardKey};
use crate::theme::{due_colors, icon, status_color};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::{Date, Status, Tone, format_due};

const COLUMN_WIDTH: Pixels = px(288.);

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
        .border_color(c.strong)
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
                .child(status_dot(status, c))
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

/// The face of one card: priority and file name, title, excerpt, labels, then a footer
/// with the due date, subtasks, work folder and assignee.
fn card_face(
    card: &Card,
    state: CardState,
    this: &Entity<Mdk>,
    today: Date,
    badge: Div,
    mono: SharedString,
    c: Colors,
) -> Stateful<Div> {
    let data = &card.data;
    let (red, amber) = due_colors(c.dark);
    // CSS collapses line breaks in the excerpt. Do the same.
    let excerpt = card
        .excerpt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let due = data.due_date.as_ref().map(|due| {
        let (label, tone) = format_due(due, today);
        div()
            .text_color(match tone {
                Tone::Red => rgb(red).into(),
                Tone::Amber => rgb(amber).into(),
                Tone::Muted => c.muted_fg,
            })
            .child(format!("{label} · {due}").to_uppercase())
    });
    let (done, total) = card.subtasks;
    let has_footer =
        !data.assignee.is_empty() || due.is_some() || !card.workspace.is_empty() || total > 0;
    let extra_labels = data.labels.len().saturating_sub(3);
    let initial = data
        .assignee
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().to_string());

    div()
        .id(SharedString::from(format!(
            "card-{}-{}",
            card.key.project, card.key.rel_path
        )))
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .border_1()
        .bg(c.card)
        .cursor_pointer()
        .map(|d| match state.focused || state.grabbed {
            // The focus ring.
            true => d.border_color(c.link),
            false => d
                .border_color(c.border)
                .hover(|s| s.border_color(c.strong).bg(c.secondary)),
        })
        .when(state.dragging, |d| d.opacity(0.4))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(badge)
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
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_right()
                        .font_family(mono.clone())
                        .text_size(px(10.))
                        .text_color(c.muted_fg)
                        .child(card.key.rel_path.clone()),
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
        .child(
            div()
                .text_size(px(15.))
                .line_height(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(data.title.clone()),
        )
        .when(!excerpt.is_empty(), |d| {
            d.child(
                div()
                    .text_xs()
                    .line_height(px(18.))
                    .text_color(c.muted_fg)
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(excerpt),
            )
        })
        .when(!data.labels.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(c.muted_fg)
                    .children(
                        data.labels
                            .iter()
                            .take(3)
                            .map(|label| div().px_1p5().bg(c.secondary).child(format!("#{label}"))),
                    )
                    .when(extra_labels > 0, |d| d.child(format!("+{extra_labels}"))),
            )
        })
        .when(has_footer, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(c.border)
                    .font_family(mono.clone())
                    .text_size(px(10.))
                    .text_color(c.muted_fg)
                    .children(due)
                    .when(total > 0, |d| d.child(format!("{done}/{total}")))
                    .when(!card.workspace.is_empty(), |d| {
                        d.child(div().truncate().child(format!("{}/", card.workspace)))
                    })
                    .child(div().flex_1())
                    .when_some(initial, |d, initial| {
                        let name = data.assignee.clone();
                        d.child(
                            div()
                                .id("card-assignee")
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .size(px(18.))
                                .bg(c.secondary)
                                .border_1()
                                .border_color(c.strong)
                                .text_color(c.fg)
                                .child(initial)
                                .tooltip(move |window, cx| {
                                    Tooltip::new(format!("@{name}")).build(window, cx)
                                }),
                        )
                    }),
            )
        })
        .when(state.grabbed, |d| {
            d.child(
                div()
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
        .left_2p5()
        .right_2p5()
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
            .gap_2()
            .p_4()
            .overflow_x_scroll()
            // A plain wheel scrolls the column under the pointer, never the board sideways.
            .restrict_scroll_to_axis()
            .track_scroll(&self.board_scroll)
            .on_drag_move(
                cx.listener(|this, ev: &DragMoveEvent<DragCard>, _, cx| {
                    this.board_drag_move(ev, cx)
                }),
            )
            .children(columns)
            // Cmd (Ctrl elsewhere) with the wheel scrolls the board sideways. It is caught in
            // the capture phase so the column under the pointer stays where it is.
            .child(
                canvas(|_, _, _| (), {
                    let scroll = self.board_scroll.clone();
                    move |_, _, window, _| {
                        let (scroll, line) = (scroll.clone(), window.line_height());
                        window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture
                                || !ev.modifiers.secondary()
                                || !scroll.bounds().contains(&ev.position)
                            {
                                return;
                            }
                            let delta = ev.delta.pixel_delta(line);
                            let mut offset = scroll.offset();
                            offset.x = (offset.x + delta.y + delta.x)
                                .clamp(-scroll.max_offset().x, px(0.));
                            scroll.set_offset(offset);
                            window.refresh();
                            cx.stop_propagation();
                        });
                    }
                })
                .absolute()
                .size_0(),
            )
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
        let card_mono = mono.clone();
        let cards = list(self.lists[i].clone(), move |row, _, cx| {
            let mono = card_mono.clone();
            let view = this.read(cx);
            let slot = view.slot == Some((i, row));
            let Some((key, _)) = view.columns[i].get(row) else {
                let this = this.clone();
                return div()
                    .id(("add-card", i))
                    .relative()
                    .px_2p5()
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
            let badge = priority_badge(card.data.priority, c, cx);
            let drag = DragCard {
                key: key.clone(),
                title: card.data.title.clone().into(),
            };
            let (open, started, key) = (this.clone(), this.clone(), key.clone());
            let dragged = key.clone();
            div()
                .relative()
                .px_2p5()
                .pb_2()
                .when(slot, |d| d.child(drop_slot(c)))
                .child(
                    card_face(card, state, &this, today, badge, mono.clone(), c)
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
            .bg(c.muted)
            .drag_over::<DragCard>(move |style, _, _, _| style.bg(c.tint))
            .on_drag_move(
                cx.listener(move |this, ev: &DragMoveEvent<DragCard>, _, cx| {
                    this.drag_move(i, ev, cx)
                }),
            )
            .on_drop(cx.listener(move |this, drag: &DragCard, _, cx| this.drop_card(i, drag, cx)))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .pl_3()
                    .pr_1p5()
                    .h(px(40.))
                    .border_t_2()
                    .border_color(status_color(status, c.dark))
                    .child(status_dot(status, c))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(status.name().to_uppercase()),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(11.))
                            .text_color(c.muted_fg)
                            .child(count.to_string()),
                    )
                    .child(div().flex_1())
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
                        icon_button(("column-add", i), "plus", c).on_click(cx.listener(
                            move |this, _, window, cx| this.open_new_card(status, window, cx),
                        )),
                    ),
            )
            .child(cards.flex_1().min_h_0())
    }
}
