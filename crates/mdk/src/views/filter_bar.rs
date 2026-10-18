//! The filter bar above the board: priority chips, due, assignee, labels and saved views.
//! Search and the Board or List switch sit in the top bar.

use super::{Colors, chip, label};
use crate::app::{FilterMenu, Mdk};
use crate::board::{DueFilter, Filters};
use crate::theme::icon;
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::Priority;

/// A small text action.
fn link(id: &'static str, text: &'static str, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(c.muted_fg)
        .cursor_pointer()
        .hover(|s| s.bg(c.secondary).text_color(c.fg))
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
            .h_7()
            .px_2()
            .bg(c.muted)
            .border_1()
            .border_color(c.border)
            .text_xs()
            .cursor_pointer()
            .hover(|s| s.border_color(c.strong))
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
                                .border_color(c.strong)
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
            chip(("priority-chip", n), on, c)
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
                .py_1()
                .border_1()
                .border_color(c.border)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c.muted_fg)
                .cursor_pointer()
                .hover(|s| s.bg(c.secondary).text_color(c.fg))
                .child(view.name.clone())
                .on_click(cx.listener(move |this, _, window, cx| this.apply_view(n, window, cx)))
                .child(
                    div()
                        .id("delete-view")
                        .px_1()
                        .hover(|s| s.text_color(c.danger))
                        .child("×")
                        .occlude()
                        .on_click(cx.listener(move |this, _, _, cx| this.delete_view(n, cx))),
                )
        });
        let views: Vec<_> = views.collect();

        let divider = || div().w_px().h_5().flex_shrink_0().bg(c.border);

        div()
            .flex()
            .flex_shrink_0()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(c.border)
            .child(label("PRIORITY", c, cx))
            .children(chips)
            .child(divider())
            .child(label("DUE", c, cx))
            .child(self.dropdown(FilterMenu::Due, due_text, due_options, c, cx))
            .child(divider())
            .child(
                div()
                    .w(px(140.))
                    .child(Input::new(&self.assignee_filter).small()),
            )
            .child(self.dropdown(FilterMenu::Labels, label_text, label_options, c, cx))
            .when(active, |d| {
                d.child(
                    link("reset-filters", "Clear filters", c).on_click(cx.listener(
                        |this, _, window, cx| this.set_filters(Filters::default(), window, cx),
                    )),
                )
            })
            .child(div().flex_1())
            .child(label("VIEWS", c, cx))
            .children(views)
            .map(|d| match &self.view_name {
                Some(name) => d.child(div().w(px(144.)).child(Input::new(name).small())),
                None => {
                    d.child(link("save-view", "Save view", c).on_click(
                        cx.listener(|this, _, window, cx| this.ask_view_name(window, cx)),
                    ))
                }
            })
    }
}
