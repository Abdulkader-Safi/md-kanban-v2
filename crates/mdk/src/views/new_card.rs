//! The New card dialog, with quick-add syntax in the title and three templates.

use super::Colors;
use crate::app::{Mdk, today};
use crate::board::NewCard;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::date_picker::{DatePicker, DatePickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::{Priority, QuickAdd, Status, Template, parse_quick_add, template};

pub struct NewCardForm {
    pub title: Entity<InputState>,
    pub body: Entity<TextareaState>,
    pub assignee: Entity<InputState>,
    pub labels: Entity<InputState>,
    pub folder: Entity<InputState>,
    pub due: Entity<DatePickerState>,
    pub status: Status,
    pub priority: Priority,
}

impl NewCardForm {
    pub fn new(status: Status, folder: &str, window: &mut Window, cx: &mut Context<Mdk>) -> Self {
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Mdk>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let title = input(
            "Card title, try: Fix login p1 #backend @safi due:friday",
            window,
            cx,
        );
        // Redraw the hint line as the title is typed. Enter in the title creates the card.
        cx.subscribe_in(
            &title,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.submit_new_card(window, cx),
                _ => {}
            },
        )
        .detach();
        let folder_text = folder.to_string();
        NewCardForm {
            title,
            body: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(4)
                    .placeholder("Description (markdown)...")
            }),
            assignee: input("name", window, cx),
            labels: input("feature, ui", window, cx),
            folder: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("auth (optional)")
                    .default_value(folder_text)
            }),
            due: cx.new(|cx| DatePickerState::new(window, cx)),
            status,
            priority: Priority::Medium,
        }
    }

    fn quick(&self, cx: &App) -> QuickAdd {
        parse_quick_add(&self.title.read(cx).value(), today())
    }

    /// The card's title: what is left of the title field once the quick-add words are taken.
    fn card_title(&self, cx: &App) -> String {
        let quick = self.quick(cx);
        if quick.title.is_empty() {
            self.title.read(cx).value().trim().to_string()
        } else {
            quick.title
        }
    }

    /// "high · @safi · due 2026-10-16 · #backend", or empty when the title sets nothing.
    fn hint(&self, cx: &App) -> String {
        let quick = self.quick(cx);
        let mut bits = Vec::new();
        bits.extend(quick.priority.map(|p| p.as_str().to_string()));
        bits.extend(quick.assignee.map(|a| format!("@{a}")));
        bits.extend(quick.due_date.map(|d| format!("due {d}")));
        bits.extend(quick.labels.iter().map(|l| format!("#{l}")));
        bits.join(" · ")
    }

    /// What to create, and the work folder to create it in. Quick-add words win over the
    /// fields, and their labels are added to the typed ones.
    pub fn input(&self, cx: &App) -> (NewCard, String) {
        let quick = self.quick(cx);
        let text = |state: &Entity<InputState>| state.read(cx).value().trim().to_string();
        let mut labels: Vec<String> = text(&self.labels)
            .split(',')
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        for label in quick.labels {
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
        let picked = self
            .due
            .read(cx)
            .date()
            .format("%Y-%m-%d")
            .map(String::from);
        let card = NewCard {
            title: self.card_title(cx),
            body: self.body.read(cx).value().to_string(),
            status: Some(self.status),
            priority: Some(quick.priority.unwrap_or(self.priority)),
            assignee: quick.assignee.unwrap_or_else(|| text(&self.assignee)),
            due_date: quick.due_date.or(picked),
            labels,
        };
        (card, text(&self.folder))
    }
}

fn field(label: &'static str, c: Colors) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .child(div().text_color(c.muted_fg).child(label))
}

/// A row of square chips where one is chosen.
fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: impl IntoIterator<Item = (T, &'static str)>,
    current: T,
    c: Colors,
    cx: &mut Context<Mdk>,
    set: impl Fn(&mut NewCardForm, T) + Copy + 'static,
) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(options.into_iter().enumerate().map(|(n, (value, name))| {
            div()
                .id((id, n))
                .px_2()
                .py_1()
                .border_1()
                .cursor_pointer()
                .map(|d| match value == current {
                    true => d
                        .bg(c.accent)
                        .text_color(c.accent_fg)
                        .border_color(c.accent),
                    false => d.border_color(c.border).hover(|s| s.bg(c.secondary)),
                })
                .child(name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(form) = &mut this.new_card {
                        set(form, value);
                    }
                    cx.notify();
                }))
        }))
}

impl Mdk {
    pub(crate) fn submit_new_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.new_card else {
            return;
        };
        let (card, folder) = form.input(cx);
        if card.title.is_empty() {
            return;
        }
        if self.create_card(card, Some(&folder), cx).is_some() {
            self.close_new_card(window, cx);
        }
    }

    fn apply_template(&mut self, kind: Template, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &mut self.new_card else {
            return;
        };
        let (priority, labels, body) = template(kind, &form.card_title(cx));
        form.priority = priority;
        form.labels.update(cx, |state, cx| {
            state.set_value(labels.join(", "), window, cx)
        });
        form.body
            .update(cx, |state, cx| state.set_value(body, window, cx));
        cx.notify();
    }

    pub(super) fn new_card_dialog(
        &self,
        form: &NewCardForm,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mono = cx.theme().mono_font_family.clone();
        let hint = form.hint(cx);
        let empty = form.card_title(cx).is_empty();
        let template_button = |id: &'static str, kind: Template, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .border_1()
                .border_color(c.border)
                .cursor_pointer()
                .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                .child(id)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.apply_template(kind, window, cx)),
                )
        };

        // A click that starts and ends on the backdrop closes the dialog. A drag that starts
        // inside the panel and ends outside does not, because the panel took the mouse down.
        div()
            .id("new-card-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().opacity(0.4))
            .on_click(cx.listener(|this, _, window, cx| this.close_new_card(window, cx)))
            .child(
                div()
                    .id("new-card")
                    .occlude()
                    .w(px(512.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(c.bg)
                    .border_1()
                    .border_color(c.border)
                    .shadow_lg()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .child("New card"),
                    )
                    .child(Input::new(&form.title))
                    .when(!hint.is_empty(), |d| {
                        d.child(
                            div()
                                .font_family(mono.clone())
                                .text_size(px(11.))
                                .text_color(c.link)
                                .child(format!("Will set: {hint}")),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .child(div().text_color(c.muted_fg).child("Template:"))
                            .child(template_button("bug", Template::Bug, cx))
                            .child(template_button("feature", Template::Feature, cx))
                            .child(template_button("chore", Template::Chore, cx)),
                    )
                    .child(Textarea::new(&form.body).font_family(mono.clone()))
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_2()
                            .child(field("Status", c).child(choice(
                                "new-status",
                                Status::ALL.map(|s| (s, s.name())),
                                form.status,
                                c,
                                cx,
                                |form, status| form.status = status,
                            )))
                            .child(field("Priority", c).child(choice(
                                "new-priority",
                                Priority::ALL.map(|p| (p, p.as_str())),
                                form.priority,
                                c,
                                cx,
                                |form, priority| form.priority = priority,
                            )))
                            .child(field("Assignee", c).child(Input::new(&form.assignee).small()))
                            .child(
                                field("Due date", c).child(
                                    DatePicker::new(&form.due)
                                        .small()
                                        .cleanable(true)
                                        .placeholder("No due date"),
                                ),
                            )
                            .child(field("Labels", c).child(Input::new(&form.labels).small()))
                            .child(
                                field("Work subfolder", c)
                                    .child(Input::new(&form.folder).small().font_family(mono)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .pt_1()
                            .child(
                                Button::new("new-card-cancel")
                                    .ghost()
                                    .small()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_new_card(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("new-card-create")
                                    .primary()
                                    .small()
                                    .label("Create card")
                                    .disabled(empty)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_new_card(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.muted_fg)
                            .child("Tip: press N anywhere to create a card."),
                    ),
            )
    }
}
