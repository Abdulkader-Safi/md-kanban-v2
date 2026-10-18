//! What the root view draws: the header, the main area and its empty states, and the
//! card sheet. The sidebar and the board have their own files.

mod board_view;
pub mod editor;
mod filter_bar;
mod list_view;
mod new_card;
mod palette;
mod sidebar;

pub use editor::CardEditor;
pub use new_card::NewCardForm;

use crate::app::{
    ArchiveDone, Confirm, ConnectFolder, DragCard, FocusDown, FocusLeft, FocusRight, FocusUp, Mdk,
    NewCardKey, OpenFocused, OpenSettings, PickUp, Redo, RescanAll, ShowBoard, ShowList,
    TogglePalette, ToggleTheme, ToggleVim, Undo,
};
use crate::board::Command;
use crate::prefs::ViewChoice;
use crate::theme::icon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::Status;

/// The theme colours the views use, copied out so they can be passed around freely.
#[derive(Clone, Copy)]
pub(crate) struct Colors {
    pub bg: Hsla,
    pub fg: Hsla,
    pub border: Hsla,
    pub muted: Hsla,
    pub muted_fg: Hsla,
    pub secondary: Hsla,
    pub card: Hsla,
    /// The lime fill of the main action.
    pub primary: Hsla,
    /// Text on the lime fill.
    pub on_primary: Hsla,
    /// The accent as text and as focus ring: lime on dark, dark green on light.
    pub link: Hsla,
    /// A faint accent fill for the chosen chip or row.
    pub tint: Hsla,
    /// The border of dialogs and outline buttons.
    pub strong: Hsla,
    /// The backdrop behind a sheet or dialog.
    pub scrim: Hsla,
    pub sidebar: Hsla,
    pub accent: Hsla,
    pub accent_fg: Hsla,
    pub danger: Hsla,
    /// The dark theme is on.
    pub dark: bool,
}

impl Colors {
    pub fn of(cx: &App) -> Colors {
        let t = cx.theme();
        Colors {
            bg: t.background,
            fg: t.foreground,
            border: t.border,
            muted: t.muted,
            muted_fg: t.muted_foreground,
            secondary: t.secondary,
            card: t.popover,
            primary: t.primary,
            on_primary: t.primary_foreground,
            link: t.link,
            tint: t.link.opacity(if t.is_dark() { 0.14 } else { 0.12 }),
            strong: rgb(if t.is_dark() { 0x3a3a41 } else { 0xb9b7ae }).into(),
            scrim: match t.is_dark() {
                true => black().opacity(0.6),
                false => black().opacity(0.35),
            },
            sidebar: t.sidebar,
            accent: t.sidebar_accent,
            accent_fg: t.sidebar_accent_foreground,
            danger: t.danger,
            dark: t.is_dark(),
        }
    }
}

/// Below this window width the sidebar moves behind a header button.
const NARROW: Pixels = px(1024.);

impl Render for Mdk {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Colors::of(cx);
        let narrow = window.viewport_size().width < NARROW;
        let font = cx.theme().font_family.clone();
        self.apply_show(window, cx);
        self.reconcile_editor(window, cx);
        // A drag that ended outside every column leaves its slot behind. Clear it.
        if !cx.has_active_drag() && (self.slot.is_some() || self.dragging.is_some()) {
            self.end_drag();
        }
        div()
            .id("mdk")
            .key_context("Mdk")
            .on_action(cx.listener(Self::close_top))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.undo(cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.redo(cx)))
            .on_action(
                cx.listener(|this, _: &TogglePalette, window, cx| this.toggle_palette(window, cx)),
            )
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.run_command(Command::Settings, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ConnectFolder, window, cx| {
                this.run_command(Command::ConnectFolder, window, cx)
            }))
            .on_action(cx.listener(|this, _: &RescanAll, window, cx| {
                this.run_command(Command::RescanAll, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.run_command(Command::ToggleTheme, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleVim, window, cx| {
                this.run_command(Command::ToggleVim, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowBoard, window, cx| {
                this.run_command(Command::BoardView, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowList, window, cx| {
                this.run_command(Command::ListView, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ArchiveDone, window, cx| {
                this.run_command(Command::ArchiveDone, window, cx)
            }))
            // A folder dropped on the window becomes a project.
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                for path in paths.paths().iter().filter(|p| p.is_dir()) {
                    this.connect(path, cx);
                }
            }))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg)
            .text_color(c.fg)
            .font_family(font)
            .text_sm()
            .when_some(self.menu_bar.clone(), |d, menu_bar| {
                d.child(div().border_b_1().border_color(c.border).child(menu_bar))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(!narrow, |d| d.child(self.sidebar(c, cx)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(self.top_bar(narrow, c, cx))
                            .child(self.main(c, cx)),
                    ),
            )
            .when(narrow && self.show_sidebar, |d| {
                d.child(
                    div()
                        .id("sidebar-backdrop")
                        .absolute()
                        .inset_0()
                        .flex()
                        .bg(c.scrim)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_sidebar = false;
                            cx.notify();
                        }))
                        .child(self.sidebar(c, cx).occlude()),
                )
            })
            .when_some(self.editor.as_ref(), |d, editor| {
                d.child(self.editor_sheet(editor, window, c, cx))
            })
            .when_some(self.new_card.as_ref(), |d, form| {
                d.child(self.new_card_dialog(form, c, cx))
            })
            .when(self.settings_open, |d| d.child(self.settings_dialog(c, cx)))
            .when(self.palette.is_some(), |d| {
                d.child(self.palette_view(c, cx))
            })
            .when_some(self.confirm.as_ref(), |d, confirm| {
                d.child(self.confirm_dialog(confirm, c, cx))
            })
            .when_some(self.toast.as_ref(), |d, toast| {
                d.child(
                    div().absolute().bottom_6().right_6().child(
                        div()
                            .id("toast")
                            .occlude()
                            .flex()
                            .items_center()
                            .gap_4()
                            .min_w(px(320.))
                            .pl_4()
                            .pr_2()
                            .py_2()
                            .bg(c.secondary)
                            .border_l_2()
                            .border_color(c.primary)
                            .child(div().flex_1().child(toast.text.clone()))
                            .when(toast.undo, |d| {
                                d.child(
                                    Button::new("toast-undo")
                                        .ghost()
                                        .small()
                                        .label("Undo")
                                        .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
                                )
                            }),
                    ),
                )
            })
    }
}

impl Render for DragCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Colors::of(cx);
        div()
            .w(px(264.))
            .p_3()
            .bg(c.card)
            .border_1()
            .border_color(c.link)
            .text_color(c.fg)
            .text_size(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .opacity(0.9)
            .child(self.title.clone())
    }
}

/// A path with the home folder written as `~`.
fn tilde(path: &std::path::Path) -> String {
    match std::env::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_owned())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

impl Mdk {
    /// The bar above the board: where you are, search, the view switch and the main actions.
    fn top_bar(&self, narrow: bool, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let has_projects = !self.board.projects.is_empty();
        let place = match &self.board.selected_project {
            _ if !has_projects => "No project open".to_string(),
            None => "All projects".to_string(),
            Some(id) => self
                .board
                .project(id)
                .map_or_else(String::new, |p| tilde(&p.root)),
        };
        let stats = format!(
            "{} cards · {} projects",
            self.board.card_count(),
            self.board.projects.len()
        );
        let dark = cx.theme().is_dark();
        let mono = cx.theme().mono_font_family.clone();
        let view = self.prefs.view;
        let tab =
            |id: &'static str, name: &'static str, choice: ViewChoice, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .h(px(48.))
                    .px_3()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .border_b_2()
                    .cursor_pointer()
                    .map(|d| match view == choice {
                        true => d.border_color(c.primary),
                        false => d
                            .border_color(c.primary.opacity(0.))
                            .text_color(c.muted_fg)
                            .hover(|s| s.text_color(c.fg)),
                    })
                    .child(name)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_view(choice, cx)))
            };

        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .h(px(48.))
            .px_4()
            .border_b_1()
            .border_color(c.border)
            .when(narrow, |d| {
                d.child(
                    icon_button("show-sidebar", "folders", c).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.show_sidebar = !this.show_sidebar;
                            cx.notify();
                        },
                    )),
                )
            })
            .child(
                div()
                    .truncate()
                    .font_family(mono.clone())
                    .text_size(px(13.))
                    .text_color(c.muted_fg)
                    .child(place),
            )
            .when(has_projects, |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .font_family(mono)
                        .text_size(px(10.))
                        .text_color(c.muted_fg)
                        .child(stats),
                )
            })
            .child(div().flex_1())
            .when(has_projects, |d| {
                d.child(
                    div().w(px(280.)).flex_shrink(1.).min_w(px(120.)).child(
                        Input::new(&self.search)
                            .small()
                            .cleanable(true)
                            .prefix(icon("search").size_4().text_color(c.muted_fg)),
                    ),
                )
                .child(
                    kbd(
                        if cfg!(target_os = "macos") {
                            "⌘K"
                        } else {
                            "Ctrl K"
                        },
                        c,
                        cx,
                    )
                    .id("search")
                    .cursor_pointer()
                    .hover(|s| s.text_color(c.fg))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_palette(window, cx))),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .child(tab("view-board", "Board", ViewChoice::Board, cx))
                        .child(tab("view-list", "List", ViewChoice::List, cx)),
                )
                .child(
                    icon_button("rescan-all", "refresh", c)
                        .on_click(cx.listener(|this, _, _, cx| this.rescan_all(cx))),
                )
            })
            .child(
                icon_button("theme", if dark { "sun" } else { "moon" }, c)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
            )
            .when(has_projects, |d| {
                d.child(
                    Button::new("new-card")
                        .primary()
                        .label("New card")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_new_card(Status::Backlog, window, cx)
                        })),
                )
            })
    }

    fn main(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let body = if self.board.projects.is_empty() {
            self.welcome(c, cx).into_any_element()
        } else if self.board.is_loading() {
            div()
                .m_4()
                .border_1()
                .border_color(c.border)
                .p_6()
                .text_color(c.muted_fg)
                .child("Loading folders...")
                .into_any_element()
        } else if self.columns.iter().all(Vec::is_empty) {
            self.no_cards(c, cx).into_any_element()
        } else if self.prefs.view == ViewChoice::List {
            self.list_view(c, cx).into_any_element()
        } else {
            self.board_view(c, cx).into_any_element()
        };
        let has_projects = !self.board.projects.is_empty();

        div()
            .id("board-area")
            .key_context("Board")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NewCardKey, window, cx| {
                this.open_new_card(Status::Backlog, window, cx)
            }))
            .on_action(cx.listener(|this, _: &PickUp, _, cx| this.pick_up_or_drop(cx)))
            .on_action(cx.listener(|this, _: &OpenFocused, window, cx| {
                if let Some(key) = this.focused.clone() {
                    this.open(key, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusUp, _, cx| this.arrow(0, -1, cx)))
            .on_action(cx.listener(|this, _: &FocusDown, _, cx| this.arrow(0, 1, cx)))
            .on_action(cx.listener(|this, _: &FocusLeft, _, cx| this.arrow(-1, 0, cx)))
            .on_action(cx.listener(|this, _: &FocusRight, _, cx| this.arrow(1, 0, cx)))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .when_some(self.notice.clone(), |d, notice| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_l_2()
                        .border_color(c.danger)
                        .bg(c.danger.opacity(0.1))
                        .px_4()
                        .py_2()
                        .child(div().flex_1().child(notice))
                        .child(icon_button("dismiss-notice", "x", c).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            },
                        ))),
                )
            })
            .when(has_projects, |d| d.child(self.filter_bar(c, cx)))
            .child(body)
    }

    /// The first-run state: no project connected yet.
    fn welcome(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        empty_state(
            "folder-open",
            "Open a folder of markdown files",
            "mdk turns every .md file in the folder into a card. Your files stay where they \
             are, in plain text, so your AI agent moves cards by editing files.",
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("welcome-connect")
                        .primary()
                        .label("Add project")
                        .on_click(cx.listener(|this, _, _, cx| this.connect_folder(cx))),
                )
                .child(
                    Button::new("welcome-sample")
                        .outline()
                        .label("Try the sample project")
                        .on_click(cx.listener(|this, _, _, cx| this.create_sample(cx))),
                ),
            c,
        )
    }

    fn no_cards(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let filtered = self.board.filters.is_active();
        empty_state(
            "search",
            "No cards match",
            match filtered {
                true => "Nothing here fits the filters that are on. Clear them to see every card.",
                false => {
                    "Pick a different project or work folder, or create your first card. \
                          Cards are plain markdown files your AI agent can read and edit."
                }
            },
            match filtered {
                true => Button::new("clear-filters")
                    .primary()
                    .label("Clear filters")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_filters(Default::default(), window, cx)
                    })),
                false => Button::new("first-card")
                    .primary()
                    .label("Create first card")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_card(Status::Backlog, window, cx)
                    })),
            },
            c,
        )
    }

    fn confirm_dialog(
        &self,
        confirm: &Confirm,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (title, message, yes) = match confirm {
            Confirm::Archive(count) => (
                format!("Archive {count} done cards?"),
                "They move to the archive/ folder.",
                Button::new("confirm-yes").primary().label("Archive"),
            ),
            Confirm::Delete(key) => {
                let title = self
                    .board
                    .card(key)
                    .map_or("this card", |c| c.data.title.as_str());
                (
                    format!("Delete \"{title}\"?"),
                    "This removes the markdown file.",
                    Button::new("confirm-yes").danger().label("Delete"),
                )
            }
        };
        let (backdrop, panel) = dialog(
            "confirm",
            440.,
            c,
            cx.listener(|this, _, window, cx| this.answer_confirm(false, window, cx)),
        );
        backdrop.items_center().child(
            panel
                .child(
                    dialog_header(title, c).child(icon_button("confirm-x", "x", c).on_click(
                        cx.listener(|this, _, window, cx| this.answer_confirm(false, window, cx)),
                    )),
                )
                .child(div().p_4().text_color(c.muted_fg).child(message))
                .child(
                    dialog_footer(c)
                        .justify_end()
                        .child(
                            Button::new("confirm-no")
                                .outline()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.answer_confirm(false, window, cx)
                                })),
                        )
                        .child(yes.on_click(cx.listener(|this, _, window, cx| {
                            this.answer_confirm(true, window, cx)
                        }))),
                ),
        )
    }
}

/// A dashed box in the middle of the main area: an icon, a title and one line of help.
fn empty_state(
    icon_name: &'static str,
    title: &'static str,
    body: &'static str,
    actions: impl IntoElement,
    c: Colors,
) -> Div {
    let inner = div()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .px_12()
        .py_10()
        .border_1()
        .border_dashed()
        .border_color(c.border)
        .text_center()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_12()
                .bg(c.tint)
                .border_1()
                .border_color(c.link)
                .child(icon(icon_name).size_6().text_color(c.link)),
        )
        .child(
            div()
                .text_size(px(28.))
                .line_height(px(34.))
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(div().max_w(px(380.)).text_color(c.muted_fg).child(body))
        .child(actions);
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .child(inner)
}

/// An uppercase mono caption: section headings and field names.
pub(crate) fn label(text: &'static str, c: Colors, cx: &App) -> Div {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c.muted_fg)
        .child(text)
}

/// A keyboard shortcut hint.
pub(crate) fn kbd(text: &'static str, c: Colors, cx: &App) -> Div {
    div()
        .flex_shrink_0()
        .px_1()
        .border_1()
        .border_color(c.strong)
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(10.))
        .text_color(c.muted_fg)
        .child(text)
}

/// The outlined priority badge: a square in the priority's colour and its name.
pub(crate) fn priority_badge(priority: mdk_format::Priority, c: Colors, cx: &App) -> Div {
    let (color, _) = crate::theme::priority_colors(priority, c.dark);
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1p5()
        .px_1p5()
        .border_1()
        .border_color(color)
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(10.))
        .text_color(color)
        .child(div().size_1p5().bg(color))
        .child(priority.as_str().to_uppercase())
}

/// The square marker of a column.
pub(crate) fn status_dot(status: Status, c: Colors) -> Div {
    div()
        .size_2()
        .flex_shrink_0()
        .bg(crate::theme::status_color(status, c.dark))
}

/// A 28px square button holding one icon.
pub(crate) fn icon_button(
    id: impl Into<ElementId>,
    icon_name: &'static str,
    c: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size_7()
        .text_color(c.muted_fg)
        .cursor_pointer()
        .hover(|s| s.bg(c.secondary).text_color(c.fg))
        .child(icon(icon_name).size_4())
}

/// A square on/off switch. Add the click handler at the call site.
pub(crate) fn toggle(id: impl Into<ElementId>, on: bool, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .w(px(34.))
        .h(px(18.))
        .px(px(3.))
        .border_1()
        .cursor_pointer()
        .map(|d| match on {
            true => d.justify_end().bg(c.primary).border_color(c.primary),
            false => d.border_color(c.strong),
        })
        .child(
            div()
                .size_3()
                .bg(if on { c.on_primary } else { c.muted_fg }),
        )
}

/// A dialog over the scrim. A click on the scrim runs `close`. Returns (scrim, panel).
pub(crate) fn dialog(
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
        .bg(c.scrim)
        .on_click(close);
    let panel = div()
        .id(id)
        .occlude()
        .w(px(width))
        .max_w_full()
        .flex()
        .flex_col()
        .bg(c.card)
        .border_1()
        .border_color(c.strong);
    (backdrop, panel)
}

/// The title row of a dialog. Add the close button as a child.
pub(crate) fn dialog_header(title: impl Into<SharedString>, c: Colors) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_2()
        .h(px(48.))
        .pl_4()
        .pr_2()
        .border_b_1()
        .border_color(c.border)
        .child(
            div()
                .flex_1()
                .truncate()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
}

/// The sunken button row at the bottom of a dialog.
pub(crate) fn dialog_footer(c: Colors) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_2()
        .px_4()
        .py_3()
        .bg(c.muted)
        .border_t_1()
        .border_color(c.border)
}

/// A form row: the field name on the left, the control on the right.
pub(crate) fn field(name: &'static str, c: Colors, cx: &App) -> Div {
    div()
        .flex()
        .items_center()
        .gap_4()
        .min_h_8()
        .child(label(name, c, cx).w(px(96.)).flex_shrink_0())
}

/// A row of square chips where one is chosen. `pick` runs with the chip that was clicked.
pub(crate) fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: impl IntoIterator<Item = (T, &'static str)>,
    current: T,
    c: Colors,
    cx: &mut Context<Mdk>,
    pick: impl Fn(&mut Mdk, T, &mut Window, &mut Context<Mdk>) + Copy + 'static,
) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(options.into_iter().enumerate().map(|(n, (value, name))| {
            chip((id, n), value == current, c)
                .child(name)
                .on_click(cx.listener(move |this, _, window, cx| pick(this, value, window, cx)))
        }))
}

/// A square chip. A chosen chip takes the accent tint.
pub(crate) fn chip(id: impl Into<ElementId>, on: bool, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .flex_shrink_0()
        .px_2()
        .py_1()
        .border_1()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|d| match on {
            true => d.bg(c.tint).border_color(c.link).text_color(c.link),
            false => d
                .border_color(c.border)
                .text_color(c.muted_fg)
                .hover(|s| s.bg(c.secondary).text_color(c.fg)),
        })
}
