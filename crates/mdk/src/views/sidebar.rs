//! The left sidebar: connect, projects, work folders, and the refresh toggles.

use super::Colors;
use crate::app::Mdk;
use crate::board::ProjectStatus;
use crate::theme::icon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

/// One selectable row. Selected rows take the teal accent.
fn row(id: impl Into<ElementId>, selected: bool, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .px_2()
        .cursor_pointer()
        .map(|d| match selected {
            true => d
                .bg(c.accent)
                .text_color(c.accent_fg)
                .font_weight(FontWeight::MEDIUM),
            false => d.hover(|s| s.bg(c.accent.opacity(0.5))),
        })
}

fn heading(text: &'static str, c: Colors) -> Div {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c.muted_fg)
        .child(text)
}

/// A small text action under the selected project. `hover` is its colour under the pointer.
fn link(
    id: &'static str,
    icon_name: &'static str,
    text: &'static str,
    hover: Hsla,
    c: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(11.))
        .text_color(c.muted_fg)
        .cursor_pointer()
        .hover(|s| s.text_color(hover))
        .child(icon(icon_name).size_3())
        .child(text)
}

impl Mdk {
    pub(super) fn sidebar(&self, c: Colors, cx: &mut Context<Self>) -> Stateful<Div> {
        let mono = cx.theme().mono_font_family.clone();
        let counts = self.board.open_counts();
        let selected = self.board.selected_project.clone();

        let projects = self.board.projects.iter().map(|project| {
            let id = project.id.clone();
            let is_selected = selected.as_deref() == Some(id.as_str());
            let ok = project.status == ProjectStatus::Ok;
            let problem = match &project.status {
                ProjectStatus::Ok => None,
                ProjectStatus::Missing => Some("Folder not found".to_string()),
                ProjectStatus::Unreadable(why) => Some(format!("Can't read folder: {why}")),
            };
            let (select, rescan, remove, locate) = (id.clone(), id.clone(), id.clone(), id.clone());

            div()
                .flex()
                .flex_col()
                .child(
                    row(SharedString::from(format!("project-{id}")), is_selected, c)
                        .py_1p5()
                        .child(div().flex_1().truncate().child(project.name.clone()))
                        .when(!ok, |d| {
                            d.child(icon("alert-triangle").size_3p5().text_color(c.danger))
                        })
                        .child(
                            div()
                                .px_1()
                                .bg(c.secondary)
                                .text_color(c.fg)
                                .text_size(px(10.))
                                .child(counts.get(id.as_str()).copied().unwrap_or(0).to_string()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_project(Some(select.clone()), cx)
                        })),
                )
                .when_some(problem, |d, problem| {
                    d.child(
                        div()
                            .px_2()
                            .pt_1()
                            .text_size(px(11.))
                            .text_color(c.danger)
                            .child(problem),
                    )
                })
                .when(is_selected, |d| {
                    d.child(
                        div()
                            .flex()
                            .gap_2()
                            .px_2()
                            .pt_1()
                            .when(ok, |d| {
                                d.child(
                                    link("project-rescan", "refresh", "rescan", c.fg, c).on_click(
                                        cx.listener(move |this, _, _, cx| this.rescan(&rescan, cx)),
                                    ),
                                )
                            })
                            .when(!ok, |d| {
                                d.child(
                                    link("project-locate", "folder-open", "Locate folder", c.fg, c)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.locate_folder(locate.clone(), cx)
                                        })),
                                )
                            })
                            .child(
                                link("project-remove", "plug-off", "remove", c.danger, c).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.remove_project(&remove, cx)
                                    }),
                                ),
                            ),
                    )
                })
        });
        let projects: Vec<_> = projects.collect();

        let workspaces = self.board.workspaces();
        let current = self.board.selected_workspace.clone();
        let folder = |name: Option<String>, cx: &mut Context<Self>| {
            let label = name.clone().unwrap_or_else(|| "all".into());
            row(
                SharedString::from(format!("folder-{label}")),
                current == name,
                c,
            )
            .py_1()
            .font_family(mono.clone())
            .text_xs()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.select_workspace(name.clone(), cx)))
        };
        let folders: Vec<_> = std::iter::once(None)
            .chain(workspaces.iter().cloned().map(Some))
            .map(|name| folder(name, cx))
            .collect();

        div()
            .id("sidebar")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(240.))
            .h_full()
            .gap_3()
            .p_3()
            .overflow_y_scroll()
            .bg(c.sidebar)
            .border_r_1()
            .border_color(c.border)
            .child(
                Button::new("connect")
                    .primary()
                    .small()
                    .w_full()
                    .icon(icon("folder-open"))
                    .label("Connect folder")
                    .on_click(cx.listener(|this, _, _, cx| this.connect_folder(cx))),
            )
            .child(heading("PROJECTS", c))
            .child(
                row("all-projects", selected.is_none(), c)
                    .py_1p5()
                    .child(icon("layout-grid").size_4())
                    .child("All projects")
                    .on_click(cx.listener(|this, _, _, cx| this.select_project(None, cx))),
            )
            .children(projects)
            .when(!workspaces.is_empty(), |d| {
                d.child(heading("WORK FOLDERS", c).mt_2())
                    .child(div().flex().flex_col().children(folders))
            })
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(c.border)
                    .child(
                        Button::new("rescan-all")
                            .outline()
                            .small()
                            .w_full()
                            .icon(icon("refresh"))
                            .label("Rescan all")
                            .on_click(cx.listener(|this, _, _, cx| this.rescan_all(cx))),
                    )
                    .child(
                        Switch::new("auto-refresh")
                            .small()
                            .checked(self.prefs.auto_refresh)
                            .label("Auto-refresh folders")
                            .on_click(
                                cx.listener(|this, on: &bool, _, cx| {
                                    this.set_auto_refresh(*on, cx)
                                }),
                            ),
                    )
                    .child(
                        Switch::new("vim-keys")
                            .small()
                            .checked(self.prefs.vim_keys)
                            .label("Vim keys in editor")
                            .on_click(
                                cx.listener(|this, on: &bool, _, cx| this.set_vim_keys(*on, cx)),
                            ),
                    )
                    .child(
                        div()
                            .font_family(mono)
                            .text_size(px(10.))
                            .text_color(c.muted_fg.opacity(0.7))
                            .child(format!(
                                "checked {}",
                                self.last_check.as_deref().unwrap_or("never")
                            )),
                    ),
            )
    }
}
