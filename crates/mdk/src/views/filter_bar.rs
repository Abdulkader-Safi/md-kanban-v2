//! The filter bar above the board: search, priority chips, assignee, labels, due, saved
//! views, and the Board or List switch.

use super::Colors;
use crate::app::{FilterMenu, Mdk};
use crate::board::{DueFilter, Filters};
use crate::prefs::ViewChoice;
use crate::theme::{icon, priority_colors};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::Priority;

/// A small text action.
fn link(id: &'static str, text: &'static str, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .text_xs()
        .text_color(c.muted_fg)
        .underline()
        .cursor_pointer()
        .hover(|s| s.text_color(c.fg))
        .child(text)
}

/// One row of a dropdown.
fn option(id: impl Into<ElementId>, text: String, selected: bool, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(c.fg)
        .cursor_pointer()
        .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
        .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
        .child(text)
}

impl Mdk {
    /// A dropdown button. Its options show below it while `menu` is the open one.
    fn dropdown(
        &self,
        menu: FilterMenu,
        label: String,
        options: Vec<Stateful<Div>>,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let open = self.filter_menu == Some(menu);
        div()
            .id(match menu {
                FilterMenu::Labels => "filter-labels",
                FilterMenu::Due => "filter-due",
            })
            .flex()
            .items_center()
            .gap_1()
            .h_8()
            .px_2()
            .border_1()
            .border_color(c.border)
            .text_xs()
            .cursor_pointer()
            .hover(|s| s.bg(c.secondary))
            .child(label)
            .child(icon("chevron-down").size_3().text_color(c.muted_fg))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.filter_menu = (this.filter_menu != Some(menu)).then_some(menu);
                cx.notify();
            }))
            .when(open, |d| {
                d.child(
                    deferred(
                        anchored().snap_to_window().child(
                            div()
                                .id("filter-options")
                                .occlude()
                                .mt_8()
                                .min_w(px(144.))
                                .max_h(px(320.))
                                .overflow_y_scroll()
                                .p_1()
                                .bg(c.card)
                                .border_1()
                                .border_color(c.border)
                                .shadow_lg()
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.filter_menu = None;
                                    cx.notify();
                                }))
                                .children(options),
                        ),
                    )
                    .with_priority(1),
                )
            })
    }

    pub(super) fn filter_bar(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let filters = &self.board.filters;
        let active = filters.is_active();

        let chips = Priority::ALL.into_iter().enumerate().map(|(n, priority)| {
            let on = filters.priorities.contains(&priority);
            let (fg, bg) = priority_colors(priority);
            div()
                .id(("priority-chip", n))
                .px_2()
                .py_1()
                .border_1()
                .text_xs()
                .cursor_pointer()
                .map(|d| match on {
                    true => d.bg(bg).text_color(fg).border_color(fg),
                    false => d.border_color(c.border).hover(|s| s.bg(c.secondary)),
                })
                .child(priority.as_str())
                .on_click(
                    cx.listener(move |this, _, _, cx| this.toggle_priority_filter(priority, cx)),
                )
        });
        let chips: Vec<_> = chips.collect();

        let labels = self.board.labels_in_view();
        let label_options: Vec<_> = std::iter::once(None)
            .chain(labels.into_iter().map(Some))
            .enumerate()
            .map(|(n, label)| {
                let text = label.clone().unwrap_or_else(|| "all labels".into());
                option(("label-option", n), text, filters.label == label, c).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.board.filters.label = label.clone();
                        this.filter_menu = None;
                        this.changed(cx);
                    },
                ))
            })
            .collect();
        let due_options: Vec<_> = DueFilter::ALL
            .into_iter()
            .enumerate()
            .map(|(n, due)| {
                option(("due-option", n), due.label().into(), filters.due == due, c).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.board.filters.due = due;
                        this.filter_menu = None;
                        this.changed(cx);
                    }),
                )
            })
            .collect();
        let label_text = filters.label.clone().unwrap_or_else(|| "all labels".into());
        let due_text = filters.due.label().to_string();

        let views = self.saved_views().iter().enumerate().map(|(n, view)| {
            div()
                .id(("saved-view", n))
                .flex()
                .items_center()
                .gap_1()
                .pl_2()
                .pr_1()
                .py_0p5()
                .bg(c.secondary)
                .text_xs()
                .cursor_pointer()
                .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                .child(view.name.clone())
                .on_click(cx.listener(move |this, _, window, cx| this.apply_view(n, window, cx)))
                .child(
                    div()
                        .id("delete-view")
                        .px_1()
                        .hover(|s| s.text_color(c.danger))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.delete_view(n, cx)
                        })),
                )
        });
        let views: Vec<_> = views.collect();

        let view = self.prefs.view;
        let segment =
            |id: &'static str, label: &'static str, choice: ViewChoice, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .px_2p5()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .map(|d| match view == choice {
                        true => d.bg(c.secondary).text_color(c.fg),
                        false => d.text_color(c.muted_fg).hover(|s| s.text_color(c.fg)),
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_view(choice, cx)))
            };

        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .w(px(288.))
                            .child(Input::new(&self.search).small().cleanable(true)),
                    )
                    .children(chips)
                    .child(
                        div()
                            .w(px(112.))
                            .child(Input::new(&self.assignee_filter).small()),
                    )
                    .child(self.dropdown(FilterMenu::Labels, label_text, label_options, c, cx))
                    .child(self.dropdown(FilterMenu::Due, due_text, due_options, c, cx))
                    .child(link("reset-filters", "reset", c).on_click(cx.listener(
                        |this, _, window, cx| this.set_filters(Filters::default(), window, cx),
                    )))
                    .child(div().px_2().py_0p5().text_xs().map(|d| match active {
                        true => d.bg(c.accent).text_color(c.accent_fg).child("filtered"),
                        false => d.bg(c.secondary).child("all"),
                    }))
                    .map(|d| match &self.view_name {
                        Some(name) => d.child(div().w(px(144.)).child(Input::new(name).small())),
                        None => d.child(link("save-view", "save view", c).on_click(
                            cx.listener(|this, _, window, cx| this.ask_view_name(window, cx)),
                        )),
                    })
                    .children(views),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .p_0p5()
                    .border_1()
                    .border_color(c.border)
                    .child(segment("view-board", "Board", ViewChoice::Board, cx))
                    .child(segment("view-list", "List", ViewChoice::List, cx)),
            )
    }
}
