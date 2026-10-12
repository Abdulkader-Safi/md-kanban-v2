//! The list view: the filtered cards as a table.

use super::{Colors, priority_badge, status_dot};
use crate::app::{ListColumn, Mdk, today};
use crate::theme::due_colors;
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::{Tone, format_due};

const COLUMNS: [(ListColumn, &str); 5] = [
    (ListColumn::Title, "TITLE"),
    (ListColumn::Status, "STATUS"),
    (ListColumn::Priority, "PRIORITY"),
    (ListColumn::Due, "DUE"),
    (ListColumn::Assignee, "ASSIGNEE"),
];

/// A table cell. The title takes the room the others leave.
fn cell(column: ListColumn) -> Div {
    match column {
        ListColumn::Title => div().flex_1().min_w_0().truncate(),
        ListColumn::Status | ListColumn::Priority => div().w(px(112.)).flex_shrink_0(),
        ListColumn::Due | ListColumn::Assignee => div().w(px(144.)).flex_shrink_0().truncate(),
    }
    .px_4()
}

impl Mdk {
    pub(super) fn list_view(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sort = self.list_sort;
        let mono = cx.theme().mono_font_family.clone();
        let header = div()
            .flex()
            .items_center()
            .py_2p5()
            .bg(c.muted)
            .font_family(mono.clone())
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c.muted_fg)
            .children(COLUMNS.into_iter().enumerate().map(|(n, (column, name))| {
                let arrow = match sort {
                    Some((sorted, descending)) if sorted == column => {
                        if descending {
                            " ↓"
                        } else {
                            " ↑"
                        }
                    }
                    _ => "",
                };
                cell(column)
                    .id(("list-header", n))
                    .cursor_pointer()
                    .hover(|s| s.text_color(c.fg))
                    .child(format!("{name}{arrow}"))
                    .on_click(cx.listener(move |this, _, _, cx| this.sort_list_by(column, cx)))
            }));

        let this = cx.entity();
        let today = today();
        let (red, amber) = due_colors(c.dark);
        let rows = uniform_list("list-rows", self.list_rows.len(), move |range, _, cx| {
            let view = this.read(cx);
            range
                .filter_map(|row| {
                    let key = view.list_rows.get(row)?;
                    let data = &view.board.card(key)?.data;
                    let focused = view.focused.as_ref() == Some(key);
                    let (due, tone) = data
                        .due_date
                        .as_deref()
                        .map_or(("—".to_string(), Tone::Muted), |d| format_due(d, today));
                    let assignee = match data.assignee.as_str() {
                        "" => "—".to_string(),
                        name => format!("@{name}"),
                    };
                    let (open, key) = (this.clone(), key.clone());
                    Some(
                        div()
                            .id(("list-row", row))
                            .flex()
                            .items_center()
                            .w_full()
                            .h_10()
                            .border_t_1()
                            .border_color(c.border)
                            .cursor_pointer()
                            .map(|d| match focused {
                                true => d.bg(c.tint),
                                false => d.hover(|s| s.bg(c.secondary)),
                            })
                            .child(
                                cell(ListColumn::Title)
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(status_dot(data.status, c))
                                    .child(div().truncate().child(data.title.clone())),
                            )
                            .child(
                                cell(ListColumn::Status)
                                    .text_xs()
                                    .text_color(c.muted_fg)
                                    .child(data.status.name()),
                            )
                            .child(cell(ListColumn::Priority).flex().child(priority_badge(
                                data.priority,
                                c,
                                cx,
                            )))
                            .child(
                                cell(ListColumn::Due)
                                    .font_family(mono.clone())
                                    .text_size(px(11.))
                                    .text_color(match tone {
                                        Tone::Red => rgb(red).into(),
                                        Tone::Amber => rgb(amber).into(),
                                        Tone::Muted => c.muted_fg,
                                    })
                                    .child(due.to_uppercase()),
                            )
                            .child(
                                cell(ListColumn::Assignee)
                                    .text_xs()
                                    .text_color(c.muted_fg)
                                    .child(assignee),
                            )
                            .on_click(move |_, window, cx| {
                                open.update(cx, |this, cx| this.open(key.clone(), window, cx))
                            }),
                    )
                })
                .collect()
        });

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .m_4()
            .bg(c.card)
            .border_1()
            .border_color(c.border)
            .child(header)
            .child(rows.flex_1().min_h_0())
    }
}
