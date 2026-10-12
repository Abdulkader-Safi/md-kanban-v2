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
    pub primary: Hsla,
    /// Teal that reads as text on the background.
    pub link: Hsla,
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
            link: t.link,
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
            .child(self.header(narrow, c, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(!narrow, |d| d.child(self.sidebar(c, cx)))
                    .child(self.main(c, cx)),
            )
            .when(narrow && self.show_sidebar, |d| {
                d.child(
                    div()
                        .id("sidebar-backdrop")
                        .absolute()
                        .inset_0()
                        .flex()
                        .bg(black().opacity(0.3))
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
                    div()
                        .absolute()
                        .bottom_4()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("toast")
                                .occlude()
                                .flex()
                                .items_center()
                                .gap_3()
                                .px_4()
                                .py_2()
                                .bg(c.card)
                                .border_1()
                                .border_color(c.border)
                                .shadow_lg()
                                .child(toast.text.clone())
                                .when(toast.undo, |d| {
                                    d.child(
                                        Button::new("toast-undo")
                                            .outline()
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
            .border_color(c.primary)
            .shadow_lg()
            .text_color(c.fg)
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .opacity(0.9)
            .child(self.title.clone())
    }
}

impl Mdk {
    fn header(&self, narrow: bool, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let stats = format!(
            "local markdown board · {} cards · {} projects",
            self.board.card_count(),
            self.board.projects.len()
        );
        let active = self
            .board
            .selected_project
            .as_ref()
            .and_then(|id| self.board.project(id))
            .map(|p| p.name.clone());
        let search = if cfg!(target_os = "macos") {
            "Search  ⌘K"
        } else {
            "Search  Ctrl K"
        };
        let dark = cx.theme().is_dark();
        let mono = cx.theme().mono_font_family.clone();

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_2p5()
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .font_weight(FontWeight::BOLD)
                    .child(icon("layout-kanban").size_5())
                    .child("mdk"),
            )
            .child(div().text_xs().text_color(c.muted_fg).child(stats))
            .when_some(active, |d, name| {
                d.child(
                    div()
                        .px_2p5()
                        .py_0p5()
                        .bg(c.secondary)
                        .font_family(mono)
                        .text_xs()
                        .child(name),
                )
            })
            .when(narrow, |d| {
                d.child(
                    Button::new("show-sidebar")
                        .outline()
                        .small()
                        .icon(icon("folders"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_sidebar = !this.show_sidebar;
                            cx.notify();
                        })),
                )
            })
            .child(div().flex_1())
            .child(
                Button::new("search")
                    .outline()
                    .small()
                    .label(search)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_palette(window, cx))),
            )
            .child(
                Button::new("theme")
                    .ghost()
                    .small()
                    .icon(icon(if dark { "sun" } else { "moon" }))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
            )
            .child(
                Button::new("new-card")
                    .primary()
                    .small()
                    .icon(icon("plus"))
                    .label("New card  N")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_card(Status::Backlog, window, cx)
                    })),
            )
    }

    fn main(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let body = if self.board.projects.is_empty() {
            self.welcome(c, cx).into_any_element()
        } else if self.board.is_loading() {
            div()
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
            .gap_3()
            .p_3()
            .when_some(self.notice.clone(), |d, notice| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_1()
                        .border_color(c.danger.opacity(0.4))
                        .bg(c.danger.opacity(0.05))
                        .px_3()
                        .py_2()
                        .child(div().flex_1().child(notice))
                        .child(
                            Button::new("dismiss-notice")
                                .ghost()
                                .xsmall()
                                .icon(icon("x"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.notice = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(has_projects, |d| d.child(self.filter_bar(c, cx)))
            .child(body)
    }

    /// The first-run state: no project connected yet.
    fn welcome(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        empty_state(c)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Connect a folder"),
            )
            .child(div().max_w(px(384.)).text_color(c.muted_fg).child(
                "Every card is a markdown file in a folder you pick, so your AI agent \
                     moves cards by editing files.",
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("welcome-connect")
                            .primary()
                            .small()
                            .icon(icon("folder-open"))
                            .label("Connect a folder")
                            .on_click(cx.listener(|this, _, _, cx| this.connect_folder(cx))),
                    )
                    .child(
                        Button::new("welcome-sample")
                            .outline()
                            .small()
                            .label("Create sample project")
                            .on_click(cx.listener(|this, _, _, cx| this.create_sample(cx))),
                    ),
            )
    }

    fn no_cards(&self, c: Colors, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        empty_state(c)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No cards match"),
            )
            .child(div().max_w(px(384.)).text_color(c.muted_fg).child(
                "Connect a project folder, pick a different project or work folder, or create \
                 your first card. Cards are plain markdown files your AI agent can read and edit.",
            ))
            .child(
                Button::new("first-card")
                    .primary()
                    .small()
                    .label("Create first card")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_card(Status::Backlog, window, cx)
                    })),
            )
    }

    fn confirm_dialog(
        &self,
        confirm: &Confirm,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (question, yes) = match confirm {
            Confirm::Archive(count) => (
                format!("Archive {count} done cards to archive/?"),
                "Archive",
            ),
            Confirm::Delete(key) => {
                let title = self
                    .board
                    .card(key)
                    .map_or("this card", |c| c.data.title.as_str());
                (
                    format!("Delete \"{title}\"? This removes the markdown file."),
                    "Delete",
                )
            }
        };
        div()
            .id("confirm-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().opacity(0.4))
            .on_click(cx.listener(|this, _, window, cx| this.answer_confirm(false, window, cx)))
            .child(
                div()
                    .id("confirm")
                    .occlude()
                    .w(px(384.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(c.bg)
                    .border_1()
                    .border_color(c.border)
                    .shadow_lg()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(question)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("confirm-no")
                                    .ghost()
                                    .small()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.answer_confirm(false, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("confirm-yes")
                                    .primary()
                                    .small()
                                    .label(yes)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.answer_confirm(true, window, cx)
                                    })),
                            ),
                    ),
            )
    }
}

fn empty_state(c: Colors) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .border_1()
        .border_dashed()
        .border_color(c.border)
        .p_10()
        .text_center()
}

/// A labelled form field.
pub(crate) fn field(label: &'static str, c: Colors) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .child(div().text_color(c.muted_fg).child(label))
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
                .on_click(cx.listener(move |this, _, window, cx| pick(this, value, window, cx)))
        }))
}
