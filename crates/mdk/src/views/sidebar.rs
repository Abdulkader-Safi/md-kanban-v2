//! The left sidebar: the brand, projects, work folders, and the refresh toggles.

use super::{Colors, label, toggle};
use crate::app::Mdk;
use crate::board::ProjectStatus;
use crate::theme::{icon, status_color};
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::Status;

/// One selectable row. The selected row has the accent edge.
fn row(id: impl Into<ElementId>, selected: bool, c: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .px_3()
        .py_2()
        .border_l_2()
        .cursor_pointer()
        .map(|d| match selected {
            true => d
                .bg(c.secondary)
                .border_color(c.primary)
                .font_weight(FontWeight::MEDIUM),
            false => d
                .border_color(c.primary.opacity(0.))
                .text_color(c.muted_fg)
                .hover(|s| s.bg(c.secondary).text_color(c.fg)),
        })
}

/// The icon at the start of a row, in the accent colour when the row is selected.
fn row_icon(name: &'static str, selected: bool, c: Colors) -> Icon {
    icon(name)
        .size_4()
        .text_color(if selected { c.link } else { c.muted_fg })
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
                        .child(row_icon("folder-open", is_selected, c))
                        .child(div().flex_1().truncate().child(project.name.clone()))
                        .when(!ok, |d| {
                            d.child(icon("alert-triangle").size_3p5().text_color(c.danger))
                        })
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_size(px(11.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(c.muted_fg)
                                .child(counts.get(id.as_str()).copied().unwrap_or(0).to_string()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_project(Some(select.clone()), cx)
                        })),
                )
                .when_some(problem, |d, problem| {
                    d.child(
                        div()
                            .px_4()
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
                            .gap_3()
                            .px_4()
                            .py_1()
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
            let text = name.clone().unwrap_or_else(|| "all".into());
            let selected = current == name;
            row(SharedString::from(format!("folder-{text}")), selected, c)
                .child(row_icon("layout-grid", selected, c))
                .child(div().flex_1().truncate().child(text))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.select_workspace(name.clone(), cx)),
                )
        };
        let folders: Vec<_> = std::iter::once(None)
            .chain(workspaces.iter().cloned().map(Some))
            .map(|name| folder(name, cx))
            .collect();
        let setting = |name: &'static str, switch: Stateful<Div>| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(c.muted_fg)
                .child(div().flex_1().child(name))
                .child(switch)
        };
        let watching = self.prefs.auto_refresh;

        div()
            .id("sidebar")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(self.prefs.sidebar_width))
            .h_full()
            .overflow_y_scroll()
            .bg(c.sidebar)
            .border_r_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2p5()
                    .h(px(48.))
                    .px_4()
                    .border_b_1()
                    .border_color(c.border)
                    .child(icon("layout-kanban").size_5().text_color(c.link))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("mdk"),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(10.))
                            .text_color(c.muted_fg)
                            .child(env!("CARGO_PKG_VERSION")),
                    ),
            )
            .child(label("PROJECTS", c, cx).px_4().pt_4().pb_1p5())
            .child(
                row("all-projects", selected.is_none(), c)
                    .child(row_icon("folders", selected.is_none(), c))
                    .child("All projects")
                    .on_click(cx.listener(|this, _, _, cx| this.select_project(None, cx))),
            )
            .children(projects)
            .child(
                row("connect", false, c)
                    .child(icon("plus").size_4())
                    .child("Add project")
                    .on_click(cx.listener(|this, _, _, cx| this.connect_folder(cx))),
            )
            .when(!workspaces.is_empty(), |d| {
                d.child(label("WORK FOLDERS", c, cx).px_4().pt_4().pb_1p5())
                    .child(div().flex().flex_col().children(folders))
            })
            .child(div().flex_1().min_h_4())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(c.border)
                    .child(setting(
                        "Auto-refresh folders",
                        toggle("auto-refresh", watching, c).on_click(cx.listener(
                            |this, _, _, cx| this.set_auto_refresh(!this.prefs.auto_refresh, cx),
                        )),
                    ))
                    .child(setting(
                        "Vim keys in editor",
                        toggle("vim-keys", self.prefs.vim_keys, c).on_click(cx.listener(
                            |this, _, _, cx| this.set_vim_keys(!this.prefs.vim_keys, cx),
                        )),
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .pt_1()
                            .child(div().size_2().bg(status_color(
                                if watching {
                                    Status::Done
                                } else {
                                    Status::Backlog
                                },
                                c.dark,
                            )))
                            .child(
                                div()
                                    .font_family(mono)
                                    .text_size(px(10.))
                                    .text_color(c.muted_fg)
                                    .child(format!(
                                        "{} · CHECKED {}",
                                        if watching { "WATCHING" } else { "NOT WATCHING" },
                                        self.last_check.as_deref().unwrap_or("NEVER")
                                    )),
                            ),
                    ),
            )
    }
}
