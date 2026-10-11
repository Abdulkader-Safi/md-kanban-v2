//! Drag and drop spike: five columns of 200 cards, GPUI's own drag events,
//! a drop slot, column highlight, and auto-scroll at the column and board edges.
//!
//! Each column is a virtualised `gpui::list` with variable-height cards.
//! The drop index comes from `ListState::bounds_for_item` on the rendered items.

use gpui_kit::base::AutoScroll;
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::time::{Duration, Instant};

pub const COLUMNS: [(&str, u32); 5] = [
    ("Backlog", 0x6b7280),
    ("To Do", 0x3b82f6),
    ("In Progress", 0xf59e0b),
    ("Review", 0x8b5cf6),
    ("Done", 0x22c55e),
];
const TEAL: u32 = 0x14b8a6;
const SLOW: Duration = Duration::from_millis(33);

#[derive(Clone)]
pub struct Card {
    pub id: usize,
    title: SharedString,
    excerpt: SharedString,
}

fn card_face(card: &Card) -> Div {
    div()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x2a2f3a))
        .bg(rgb(0x171a21))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .child(card.title.clone()),
        )
        .when(!card.excerpt.is_empty(), |d| {
            d.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child(card.excerpt.clone()),
            )
        })
}

/// The drag payload. It also renders the preview that follows the pointer.
#[derive(Clone)]
struct DragCard(Card);

impl Render for DragCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        card_face(&self.0)
            .w(px(264.))
            .text_color(rgb(0xe5e7eb))
            .opacity(0.9)
    }
}

pub struct DndSpike {
    pub columns: Vec<Vec<Card>>,
    pub lists: Vec<ListState>,
    board_scroll: ScrollHandle,
    /// Where the dragged card would land: (column, index).
    pub slot: Option<(usize, usize)>,
    dragging: Option<usize>,
    v_auto: AutoScroll,
    h_auto: AutoScroll,
    auto_col: usize,
    last_frame: Option<Instant>,
    frames: usize,
    slow_frames: usize,
    worst_frame: Duration,
}

impl DndSpike {
    pub fn new(per_column: usize) -> Self {
        let columns: Vec<Vec<Card>> = (0..COLUMNS.len())
            .map(|c| {
                (0..per_column)
                    .map(|i| {
                        let id = c * per_column + i;
                        Card {
                            id,
                            title: format!("Card {id}: {}", "fix the thing ".repeat(1 + id % 3))
                                .into(),
                            excerpt: "Some notes about this card. ".repeat(id % 4).into(),
                        }
                    })
                    .collect()
            })
            .collect();
        let lists = columns
            .iter()
            .map(|col| ListState::new(col.len(), ListAlignment::Top, px(400.)))
            .collect();
        Self {
            columns,
            lists,
            board_scroll: ScrollHandle::new(),
            slot: None,
            dragging: None,
            v_auto: AutoScroll::default(),
            h_auto: AutoScroll::default(),
            auto_col: 0,
            last_frame: None,
            frames: 0,
            slow_frames: 0,
            worst_frame: Duration::ZERO,
        }
    }

    /// The pointer moved during a drag. Every column hears this, so check the bounds.
    fn drag_move(&mut self, c: usize, ev: &DragMoveEvent<DragCard>, cx: &mut Context<Self>) {
        let pos = ev.event.position;
        if !ev.bounds.contains(&pos) {
            if self.slot.is_some_and(|(col, _)| col == c) {
                self.slot = None;
                self.v_auto.set(None, cx, |_, _: &mut Self, _| {});
                cx.notify();
            }
            return;
        }

        let list = &self.lists[c];
        let len = self.columns[c].len();
        let mut index = len;
        for i in list.logical_scroll_top().item_ix..len {
            match list.bounds_for_item(i) {
                Some(b) if pos.y < b.center().y => {
                    index = i;
                    break;
                }
                Some(_) => {}
                None => {
                    index = i;
                    break;
                }
            }
        }
        if self.slot != Some((c, index)) {
            self.slot = Some((c, index));
            cx.notify();
        }

        self.auto_col = c;
        let delta = AutoScroll::compute_delta(pos.y, list.viewport_bounds());
        self.v_auto.set(delta, cx, |d, this: &mut Self, cx| {
            this.lists[this.auto_col].scroll_by(d);
            cx.notify();
        });
    }

    fn board_drag_move(&mut self, ev: &DragMoveEvent<DragCard>, cx: &mut Context<Self>) {
        // compute_delta works on y, so hand it the board's x range as if it were vertical.
        let sideways = Bounds::new(
            point(px(0.), ev.bounds.left()),
            size(px(0.), ev.bounds.size.width),
        );
        let delta = AutoScroll::compute_delta(ev.event.position.x, sideways);
        self.h_auto.set(delta, cx, |d, this: &mut Self, cx| {
            let (offset, max) = (this.board_scroll.offset(), this.board_scroll.max_offset());
            let x = (offset.x - d).clamp(-max.x, px(0.));
            this.board_scroll.set_offset(point(x, offset.y));
            cx.notify();
        });
    }

    fn drop_card(&mut self, c: usize, drag: &DragCard, cx: &mut Context<Self>) {
        let id = drag.0.id;
        let Some((from_col, from)) = self.columns.iter().enumerate().find_map(|(col, cards)| {
            cards
                .iter()
                .position(|card| card.id == id)
                .map(|i| (col, i))
        }) else {
            return;
        };
        let mut index = match self.slot {
            Some((col, i)) if col == c => i,
            _ => self.columns[c].len(),
        };
        let card = self.columns[from_col].remove(from);
        self.lists[from_col].splice(from..from + 1, 0);
        if from_col == c && from < index {
            index -= 1;
        }
        self.columns[c].insert(index, card);
        self.lists[c].splice(index..index, 1);
        self.end_drag();
        cx.notify();
    }

    fn end_drag(&mut self) {
        self.slot = None;
        self.dragging = None;
        self.v_auto.stop();
        self.h_auto.stop();
    }

    /// Frame gaps while a drag is active. The exit criterion is no gap over 33 ms.
    fn measure(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            window.request_animation_frame();
            let now = Instant::now();
            match self.last_frame.replace(now) {
                None => (self.frames, self.slow_frames, self.worst_frame) = (0, 0, Duration::ZERO),
                Some(last) => {
                    let gap = now - last;
                    self.frames += 1;
                    self.worst_frame = self.worst_frame.max(gap);
                    self.slow_frames += (gap > SLOW) as usize;
                }
            }
        } else if self.last_frame.take().is_some() {
            // The drag ended, by a drop or by a release outside any column.
            self.end_drag();
            eprintln!("drag: {}", self.stats());
        }
    }

    fn stats(&self) -> String {
        format!(
            "{} frames, worst {:.1} ms, {} over 33 ms",
            self.frames,
            self.worst_frame.as_secs_f64() * 1000.,
            self.slow_frames
        )
    }

    fn column(&self, c: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (name, color) = COLUMNS[c];
        let this = cx.entity();
        let cards = list(self.lists[c].clone(), move |i, _, cx| {
            let (card, slot, dragging, last) = {
                let t = this.read(cx);
                let col = &t.columns[c];
                (col[i].clone(), t.slot, t.dragging, i + 1 == col.len())
            };
            let slot_line = || div().absolute().left_2().right_2().h(px(2.)).bg(rgb(TEAL));
            let (id, entity) = (card.id, this.clone());
            div()
                .id(("card", id))
                .relative()
                .px_2()
                .pb_2()
                .when(slot == Some((c, i)), |d| d.child(slot_line().top(px(-5.))))
                .when(last && slot == Some((c, i + 1)), |d| {
                    d.child(slot_line().bottom(px(3.)))
                })
                .child(card_face(&card).when(dragging == Some(id), |d| d.opacity(0.4)))
                .on_drag(DragCard(card), move |drag, _, _, cx| {
                    entity.update(cx, |t, cx| {
                        t.dragging = Some(id);
                        cx.notify();
                    });
                    cx.new(|_| drag.clone())
                })
                .into_any_element()
        });

        div()
            .id(("column", c))
            .w(px(288.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(rgb(0x0f1218))
            .drag_over::<DragCard>(|style, _, _, _| style.bg(rgb(0x1a2130)))
            .on_drag_move(
                cx.listener(move |this, ev: &DragMoveEvent<DragCard>, _, cx| {
                    this.drag_move(c, ev, cx)
                }),
            )
            .on_drop(cx.listener(move |this, drag: &DragCard, _, cx| this.drop_card(c, drag, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .child(div().size_2().rounded_full().bg(rgb(color)))
                    .child(name)
                    .child(
                        div()
                            .text_color(rgb(0x9ca3af))
                            .child(self.columns[c].len().to_string()),
                    ),
            )
            .child(cards.flex_1())
    }
}

impl Render for DndSpike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.measure(window, cx);
        let columns: Vec<_> = (0..COLUMNS.len()).map(|c| self.column(c, cx)).collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0a0c10))
            .text_color(rgb(0xe5e7eb))
            .child(div().px_3().pt_2().text_sm().child(self.stats()))
            .child(
                div()
                    .id("board")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap_3()
                    .p_3()
                    .overflow_x_scroll()
                    .track_scroll(&self.board_scroll)
                    .on_drag_move(cx.listener(|this, ev: &DragMoveEvent<DragCard>, _, cx| {
                        this.board_drag_move(ev, cx)
                    }))
                    .children(columns),
            )
    }
}
