//! The card editor: a side sheet with the card's fields, a markdown editor with a
//! formatting toolbar, and a preview.
//!
//! The form is compared with the card as it was loaded. Only the fields that differ are
//! written, on top of whatever is on disk at that moment, so an agent's edit to another
//! field of the same card survives. If the file changes while the form has unsaved edits,
//! nothing is written until the user picks a side.

use super::{Colors, choice, field};
use crate::app::{Confirm, Mdk};
use crate::board::{Card, CardKey};
use crate::prefs::EditorTab;
use crate::theme::icon;
use chrono::NaiveDate;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::calendar::Date as PickerDate;
use gpui_kit::component::date_picker::{DatePicker, DatePickerEvent, DatePickerState};
use gpui_kit::component::input::{
    Backspace, Delete, Editor, EditorState, Enter, Escape, IndentInline, Input, InputEvent,
    InputState, OutdentInline, Redo, Undo,
};
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use mdk_format::{CardPatch, Priority, Status};
use mdk_markdown::commands::{self, Edit, ListKind};
use mdk_vim::{Handled, Mode, Vim, VimBuffer};
use std::ops::Range;
use std::time::Duration;

actions!(md_editor, [FormatBold, FormatItalic, FormatCode]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-b", FormatBold, Some("MdEditor")),
        KeyBinding::new("secondary-i", FormatItalic, Some("MdEditor")),
        KeyBinding::new("secondary-e", FormatCode, Some("MdEditor")),
    ]);
}

const AUTOSAVE: Duration = Duration::from_millis(500);

/// One formatting action of the toolbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Heading(usize),
    Wrap(&'static str),
    Link { image: bool },
    List(ListKind),
    CodeBlock,
    Table,
    Callout,
    Math,
    Mermaid,
    Rule,
}

impl Tool {
    fn run(self, text: &str, selection: Range<usize>) -> Edit {
        match self {
            Tool::Heading(level) => commands::heading(text, selection, level),
            Tool::Wrap("`") => commands::wrap(text, selection, "`", "`", "code"),
            Tool::Wrap(mark) => commands::wrap(text, selection, mark, mark, "text"),
            Tool::Link { image } => commands::link(text, selection, image),
            Tool::List(kind) => commands::toggle_list(text, selection, kind),
            Tool::CodeBlock => commands::code_block(text, selection),
            Tool::Table => commands::table(text, selection),
            Tool::Callout => commands::callout(text, selection),
            Tool::Math => commands::math_block(text, selection),
            Tool::Mermaid => commands::mermaid(text, selection),
            Tool::Rule => commands::rule(text, selection),
        }
    }
}

/// The toolbar: icon, tooltip, action, and whether a new group starts here.
const TOOLS: [(&str, &str, Tool, bool); 20] = [
    ("h-1", "Heading 1", Tool::Heading(1), false),
    ("h-2", "Heading 2", Tool::Heading(2), false),
    ("h-3", "Heading 3", Tool::Heading(3), false),
    ("bold", "Bold (⌘B)", Tool::Wrap("**"), true),
    ("italic", "Italic (⌘I)", Tool::Wrap("_"), false),
    ("strikethrough", "Strikethrough", Tool::Wrap("~~"), false),
    ("highlight", "Highlight", Tool::Wrap("=="), false),
    ("code", "Inline code (⌘E)", Tool::Wrap("`"), false),
    ("link", "Link", Tool::Link { image: false }, true),
    ("photo", "Image", Tool::Link { image: true }, false),
    ("list", "Bulleted list", Tool::List(ListKind::Bullet), true),
    (
        "list-numbers",
        "Numbered list",
        Tool::List(ListKind::Ordered),
        false,
    ),
    ("list-check", "Checklist", Tool::List(ListKind::Task), false),
    ("blockquote", "Quote", Tool::List(ListKind::Quote), false),
    ("source-code", "Code block", Tool::CodeBlock, true),
    ("table", "Table", Tool::Table, false),
    ("info-square-rounded", "Callout", Tool::Callout, false),
    ("math-function", "Math block", Tool::Math, false),
    ("binary-tree", "Mermaid diagram", Tool::Mermaid, false),
    ("separator-horizontal", "Divider", Tool::Rule, false),
];

pub struct CardEditor {
    pub key: CardKey,
    /// The card as the form last loaded it or saved it. Unsaved edits are the difference
    /// between the form and this.
    base: Card,
    pub title: Entity<InputState>,
    pub assignee: Entity<InputState>,
    pub labels: Entity<InputState>,
    pub due: Entity<DatePickerState>,
    pub body: Entity<EditorState>,
    pub status: Status,
    pub priority: Priority,
    /// The frontmatter is broken, so the body editor holds the whole file to fix by hand.
    pub raw_mode: bool,
    /// The file changed on disk while the form had unsaved edits.
    pub conflict: bool,
    /// The file is gone from disk.
    pub deleted: bool,
    raw_open: bool,
    /// Vim's state for this card. Used only while "Vim keys in editor" is on.
    pub vim: Vim,
    save_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

fn parse_date(due: Option<&str>) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(due?, "%Y-%m-%d").ok()
}

fn split_labels(text: &str) -> Vec<String> {
    text.split(',')
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

impl CardEditor {
    fn new(card: &Card, window: &mut Window, cx: &mut Context<Mdk>) -> CardEditor {
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Mdk>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let title = input("Card title", window, cx);
        let assignee = input("name", window, cx);
        let labels = input("feature, ui", window, cx);
        let due = cx.new(|cx| DatePickerState::new(window, cx));
        let body = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .soft_wrap(true)
                .line_number(false)
                .searchable(true)
                .placeholder("Write markdown...")
        });

        let on_input = |this: &mut Mdk,
                        _: &Entity<InputState>,
                        event: &InputEvent,
                        window: &mut Window,
                        cx: &mut Context<Mdk>| {
            if matches!(event, InputEvent::Change) {
                this.editor_changed(window, cx);
            }
        };
        let subscriptions = vec![
            cx.subscribe_in(&title, window, on_input),
            cx.subscribe_in(&assignee, window, on_input),
            cx.subscribe_in(&labels, window, on_input),
            cx.subscribe_in(&body, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.editor_changed(window, cx);
                }
            }),
            cx.subscribe_in(&due, window, |this, _, _: &DatePickerEvent, window, cx| {
                this.editor_changed(window, cx)
            }),
        ];

        let mut editor = CardEditor {
            key: card.key.clone(),
            base: card.clone(),
            title,
            assignee,
            labels,
            due,
            body,
            status: card.data.status,
            priority: card.data.priority,
            raw_mode: false,
            conflict: false,
            deleted: false,
            raw_open: false,
            vim: Vim::new(),
            save_task: None,
            _subscriptions: subscriptions,
        };
        editor.load(card, window, cx);
        editor
    }

    /// Fill the form from a card, dropping any unsaved edits.
    fn load(&mut self, card: &Card, window: &mut Window, cx: &mut Context<Mdk>) {
        let data = &card.data;
        self.base = card.clone();
        self.status = data.status;
        self.priority = data.priority;
        self.raw_mode = card.data.parse_error.is_some();
        self.conflict = false;
        self.deleted = false;
        let body = if self.raw_mode {
            card.raw.to_string()
        } else {
            data.body.clone()
        };
        let set = |state: &Entity<InputState>,
                   value: String,
                   window: &mut Window,
                   cx: &mut Context<Mdk>| {
            if state.read(cx).value() != value {
                state.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        };
        set(&self.title, data.title.clone(), window, cx);
        set(&self.assignee, data.assignee.clone(), window, cx);
        set(&self.labels, data.labels.join(", "), window, cx);
        if self.body.read(cx).value() != body {
            self.body
                .update(cx, |state, cx| state.set_value(body, window, cx));
        }
        let due = parse_date(data.due_date.as_deref());
        if self.picked_date(cx) != due {
            self.due.update(cx, |state, cx| {
                state.set_date(PickerDate::Single(due), window, cx)
            });
        }
    }

    fn picked_date(&self, cx: &App) -> Option<NaiveDate> {
        self.due.read(cx).date().start()
    }

    /// The whole file text, when fixing broken frontmatter by hand.
    fn raw_text(&self, cx: &App) -> String {
        self.body.read(cx).value().to_string()
    }

    /// What the form changes about the card it was loaded from. Empty when nothing is unsaved.
    fn patch(&self, cx: &App) -> CardPatch {
        let base = &self.base.data;
        let mut patch = CardPatch::default();
        let title = self.title.read(cx).value().trim().to_string();
        if !title.is_empty() && title != base.title {
            patch.title = Some(title);
        }
        let body = self.body.read(cx).value().replace("\r\n", "\n");
        if body.trim() != base.body {
            patch.body = Some(body.trim().to_string());
        }
        if self.status != base.status {
            patch.status = Some(self.status);
        }
        if self.priority != base.priority {
            patch.priority = Some(self.priority);
        }
        let assignee = self.assignee.read(cx).value().trim().to_string();
        if assignee != base.assignee {
            patch.assignee = Some(assignee);
        }
        // Compare as dates, so a due value the picker cannot show is left alone.
        let due = self.picked_date(cx);
        if due != parse_date(base.due_date.as_deref()) {
            patch.due_date = Some(due.map(|d| d.format("%Y-%m-%d").to_string()));
        }
        let labels = split_labels(&self.labels.read(cx).value());
        if labels != base.labels {
            patch.labels = Some(labels);
        }
        patch
    }

    fn is_dirty(&self, cx: &App) -> bool {
        if self.raw_mode {
            self.raw_text(cx) != *self.base.raw
        } else {
            self.patch(cx) != CardPatch::default()
        }
    }

    /// The exact file text a save would write now.
    fn preview_file(&self, cx: &App) -> String {
        if self.raw_mode {
            return self.raw_text(cx);
        }
        let file_name = self
            .key
            .rel_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.key.rel_path);
        let patch = self.patch(cx);
        if patch == CardPatch::default() {
            return self.base.raw.to_string();
        }
        mdk_format::apply_patch(&self.base.raw, file_name, &patch, &mdk_format::now_iso())
            .unwrap_or_else(|_| self.base.raw.to_string())
    }
}

/// gpui-kit's editor, as the buffer Vim edits.
struct EditorBuffer<'a, 'b> {
    state: &'a mut EditorState,
    window: &'a mut Window,
    cx: &'a mut Context<'b, EditorState>,
}

impl VimBuffer for EditorBuffer<'_, '_> {
    fn text(&self) -> String {
        self.state.value().to_string()
    }

    fn cursor(&self) -> usize {
        self.state.cursor()
    }

    fn set_cursor(&mut self, pos: usize) {
        self.state.set_selected_range(pos..pos, self.cx);
    }

    fn set_selection(&mut self, range: Range<usize>) {
        self.state.set_selected_range(range, self.cx);
    }

    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.state.set_selected_range(range, self.cx);
        self.state.replace(text.to_string(), self.window, self.cx);
    }

    // The editor's history is private, so undo and redo go through its own actions.
    fn undo(&mut self) {
        self.window.dispatch_action(Box::new(Undo), self.cx);
    }

    fn redo(&mut self) {
        self.window.dispatch_action(Box::new(Redo), self.cx);
    }
}

/// The key as Vim names it: the character typed, or the key's name.
fn vim_key(keystroke: &Keystroke) -> Option<String> {
    let m = keystroke.modifiers;
    if m.platform || m.alt || m.function {
        return None;
    }
    if m.control {
        return (keystroke.key == "r").then(|| "ctrl-r".to_string());
    }
    match keystroke.key_char.as_deref() {
        Some(typed) if typed.chars().count() == 1 && !typed.chars().any(char::is_control) => {
            Some(typed.to_string())
        }
        _ => match keystroke.key.as_str() {
            "escape" | "enter" | "backspace" => Some(keystroke.key.clone()),
            "space" => Some(" ".to_string()),
            key if key.chars().count() == 1 => Some(if m.shift {
                key.to_uppercase()
            } else {
                key.to_string()
            }),
            _ => None,
        },
    }
}

/// Replace the editor's text with `edit.text` by changing only the part that differs, so
/// the change is one undo step, then select `edit.selection`.
fn apply_edit(
    state: &mut EditorState,
    edit: Edit,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) {
    let old = state.value().to_string();
    let prefix = old
        .char_indices()
        .zip(edit.text.chars())
        .find(|((_, a), b)| a != b)
        .map_or(old.len().min(edit.text.len()), |((i, _), _)| i);
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(edit.text[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum::<usize>();
    state.set_selected_range(prefix..old.len() - suffix, cx);
    state.replace(
        edit.text[prefix..edit.text.len() - suffix].to_string(),
        window,
        cx,
    );
    state.set_selected_range(edit.selection, cx);
}

impl Mdk {
    /// Open a card in the editor sheet. Unsaved edits to the card open before are saved first.
    pub fn open(&mut self, key: CardKey, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_for = None;
        self.flush_editor(window, cx);
        let Some(card) = self.board.card(&key).cloned() else {
            return;
        };
        if let Some(why) = &card.unreadable {
            self.show_toast(format!("Can't open this file: {why}"), false, cx);
            return;
        }
        self.editor = Some(CardEditor::new(&card, window, cx));
        self.open_card = Some(key);
        cx.notify();
    }

    /// Save now if a save is pending, then close the sheet.
    pub fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_editor(window, cx);
        self.editor = None;
        self.open_card = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Something in the form changed: save 500 ms after the last change.
    fn editor_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE).await;
            this.update_in(cx, |this, window, cx| this.save_editor(window, cx))
                .ok();
        }));
        cx.notify();
    }

    /// Write a pending save right away, so no edit is lost or lands in another card's file.
    pub fn flush_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(|e| e.save_task.is_some()) {
            self.save_editor(window, cx);
        }
    }

    /// Write the form's unsaved edits. Does nothing while a clash is waiting for an answer.
    pub fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.save_task = None;
        if editor.conflict || editor.deleted {
            return;
        }
        let key = editor.key.clone();
        if editor.raw_mode {
            let text = editor.raw_text(cx);
            if text != *editor.base.raw {
                let written = self.board.write_raw(&key, &text);
                self.after_save(written, None, window, cx);
            }
            return;
        }
        let mut patch = editor.patch(cx);
        if patch == CardPatch::default() {
            return;
        }
        if let Some(title) = &patch.title {
            // The title lives in the body's first heading. Change it there too, in the
            // editor as well as the file, or the next save would put the old heading back.
            let body = patch
                .body
                .clone()
                .unwrap_or_else(|| editor.base.data.body.clone());
            let retitled = mdk_format::set_title(&body, title);
            if retitled != body {
                let text = retitled.clone();
                editor
                    .body
                    .update(cx, |state, cx| state.set_value(text, window, cx));
                patch.body = Some(retitled);
            }
        }
        let saved = self.board.update_card(&key, &patch);
        self.after_save(saved, Some(patch), window, cx);
    }

    fn after_save(
        &mut self,
        result: Result<(), String>,
        patch: Option<CardPatch>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Err(error) => {
                // The edit stays in the form, so the next change tries again.
                self.show_toast(format!("Could not save: {error}"), false, cx);
            }
            Ok(()) => {
                let card = self
                    .editor
                    .as_ref()
                    .and_then(|e| self.board.card(&e.key))
                    .cloned();
                if let (Some(editor), Some(card)) = (&mut self.editor, card) {
                    let was_raw = editor.raw_mode;
                    editor.base = card.clone();
                    if was_raw && card.data.parse_error.is_none() {
                        // The YAML is fixed. Back to the normal form.
                        editor.load(&card, window, cx);
                    } else if !was_raw && patch.is_some_and(|p| p.title.is_none()) {
                        // The heading may have been edited in the body. Show the new title.
                        let title = card.data.title.clone();
                        if editor.title.read(cx).value() != title {
                            editor
                                .title
                                .update(cx, |state, cx| state.set_value(title, window, cx));
                        }
                    }
                }
            }
        }
        self.changed(cx);
    }

    /// Bring the open editor in line with the board after the card's file changed on disk.
    /// With no unsaved edits the new content loads silently. With unsaved edits a banner asks.
    pub(crate) fn reconcile_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        match self.board.card(&editor.key).cloned() {
            None => editor.deleted = true,
            Some(card) => {
                editor.deleted = false;
                if card.content_hash != editor.base.content_hash && !editor.conflict {
                    if editor.is_dirty(cx) {
                        editor.conflict = true;
                    } else {
                        editor.load(&card, window, cx);
                    }
                }
            }
        }
    }

    /// Answer the "Changed on disk" banner.
    pub fn resolve_conflict(
        &mut self,
        keep_mine: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(card) = self.board.card(&editor.key).cloned() else {
            return;
        };
        editor.conflict = false;
        if keep_mine {
            // My changed fields go on top of their file. Their other fields stay.
            if editor.raw_mode {
                let (key, text) = (editor.key.clone(), editor.raw_text(cx));
                let written = self.board.write_raw(&key, &text);
                self.after_save(written, None, window, cx);
            } else {
                let (key, patch) = (editor.key.clone(), editor.patch(cx));
                let saved = self.board.update_card(&key, &patch);
                self.after_save(saved, Some(patch), window, cx);
            }
        } else {
            editor.load(&card, window, cx);
        }
        cx.notify();
    }

    /// Write the deleted file back as it last was.
    pub fn recreate_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let (key, text) = (editor.key.clone(), editor.base.raw.to_string());
        editor.deleted = false;
        let written = self.board.write_raw(&key, &text);
        self.after_save(written, None, window, cx);
    }

    /// Run a toolbar action on the body editor.
    pub fn format(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        editor.body.update(cx, |state, cx| {
            let edit = tool.run(&state.value(), state.selected_range());
            apply_edit(state, edit, window, cx);
            state.focus(window, cx);
        });
    }

    /// Whether Vim owns the body editor's keys right now.
    fn vim_on(&self) -> bool {
        self.prefs.vim_keys && self.editor.as_ref().is_some_and(|e| !e.raw_mode)
    }

    /// Give a key to Vim. Returns true when Vim used it, so the editor must not.
    pub fn vim_handle(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.vim_on() {
            return false;
        }
        let Some(editor) = &mut self.editor else {
            return false;
        };
        let (vim, body) = (&mut editor.vim, editor.body.clone());
        let handled = body.update(cx, |state, cx| {
            vim.handle(key, &mut EditorBuffer { state, window, cx })
        });
        cx.notify();
        match handled {
            Handled::Yes => true,
            Handled::No => false,
            Handled::Close => {
                self.close_editor(window, cx);
                true
            }
        }
    }

    /// Outside insert mode, a key the editor has bound (Enter, Backspace, Tab, Delete) must
    /// not reach it. `key` is what Vim should see, if anything.
    fn vim_blocks(
        &mut self,
        key: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.vim_on()
            || self
                .editor
                .as_ref()
                .is_some_and(|e| e.vim.mode() == Mode::Insert)
        {
            return false;
        }
        if let Some(key) = key {
            self.vim_handle(key, window, cx);
        }
        true
    }

    /// Enter in the body: continue the list the cursor is in. False when it is not in one.
    fn continue_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(editor) = self.editor.as_ref().filter(|e| !e.raw_mode) else {
            return false;
        };
        editor.body.update(cx, |state, cx| {
            match commands::continue_list(&state.value(), state.selected_range()) {
                Some(edit) => {
                    apply_edit(state, edit, window, cx);
                    true
                }
                None => false,
            }
        })
    }

    fn set_editor_tab(&mut self, tab: EditorTab, cx: &mut Context<Self>) {
        self.prefs.editor_tab = tab;
        self.prefs_changed(cx);
    }

    /// The prompt Electron copies, for handing the card to an agent.
    pub fn ai_prompt(&self) -> Option<String> {
        let card = self.board.card(self.open_card.as_ref()?)?;
        let project = &self.board.project(&card.key.project)?.name;
        let data = &card.data;
        let or = |text: &str, fallback: &str| {
            if text.is_empty() {
                fallback.to_string()
            } else {
                text.to_string()
            }
        };
        Some(format!(
            "Work on this kanban task.\n\nFile: {project}/{}\nTitle: {}\nStatus: {}\nPriority: {}\nAssignee: {}\nLabels: {}\n\n{}\n\nRules: read the file, implement the work, then update the same markdown file (move status, update body checkboxes). Keep YAML frontmatter keys intact.",
            card.key.rel_path,
            data.title,
            data.status.as_str(),
            data.priority.as_str(),
            or(&data.assignee, "unassigned"),
            or(&data.labels.join(", "), "none"),
            data.body,
        ))
    }

    fn copy_ai_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_editor(window, cx);
        if let Some(prompt) = self.ai_prompt() {
            cx.write_to_clipboard(ClipboardItem::new_string(prompt));
            self.show_toast("Copied", false, cx);
        }
    }

    fn banner(&self, text: &'static str, c: Colors) -> Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .border_1()
            .border_color(c.danger.opacity(0.4))
            .bg(c.danger.opacity(0.08))
            .child(icon("alert-triangle").size_4())
            .child(div().flex_1().child(text))
    }

    pub(super) fn editor_sheet(
        &self,
        editor: &CardEditor,
        window: &Window,
        c: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mono = cx.theme().mono_font_family.clone();
        let project = self
            .board
            .project(&editor.key.project)
            .map_or(String::new(), |p| p.name.clone());
        let expanded = self.prefs.editor_expanded;
        let width = if expanded {
            window.viewport_size().width * (2. / 3.)
        } else {
            px(672.)
        };
        let tab = self.prefs.editor_tab;
        let saving = editor.save_task.is_some();
        let listener_button =
            |id: &'static str, label: &'static str| Button::new(id).small().label(label);

        let tab_button =
            |id: &'static str, label: &'static str, this_tab: EditorTab, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .px_2p5()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .map(|d| match tab == this_tab {
                        true => d.bg(c.secondary).text_color(c.fg),
                        false => d.text_color(c.muted_fg).hover(|s| s.text_color(c.fg)),
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_editor_tab(this_tab, cx)))
            };

        let toolbar =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_0p5()
                .p_1()
                .border_b_1()
                .border_color(c.border)
                .children(
                    TOOLS
                        .iter()
                        .enumerate()
                        .map(|(n, (name, label, tool, gap))| {
                            let (tool, label) = (*tool, *label);
                            div()
                                .id(("tool", n))
                                .when(*gap, |d| d.ml_2())
                                .p_1()
                                .text_color(c.muted_fg)
                                .cursor_pointer()
                                .hover(|s| s.bg(c.accent).text_color(c.accent_fg))
                                .child(icon(name).size_4())
                                .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.format(tool, window, cx)
                                }))
                        }),
                );

        let markdown = div()
            .key_context("MdEditor")
            .on_action(cx.listener(|this, _: &FormatBold, window, cx| {
                this.format(Tool::Wrap("**"), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatItalic, window, cx| {
                this.format(Tool::Wrap("_"), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatCode, window, cx| {
                this.format(Tool::Wrap("`"), window, cx)
            }))
            // Vim sees plain keys before the editor does. GPUI runs key bindings before
            // key-down listeners, so the keys the editor has bound are caught as actions.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if let Some(key) = vim_key(&event.keystroke)
                    && this.vim_handle(&key, window, cx)
                {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                // With Vim on, Escape belongs to Vim and never closes the card.
                if this.vim_on() {
                    this.vim_handle("escape", window, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Backspace, window, cx| {
                if this.vim_blocks(Some("backspace"), window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Delete, window, cx| {
                if this.vim_blocks(None, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                if this.vim_blocks(None, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &OutdentInline, window, cx| {
                if this.vim_blocks(None, window, cx) {
                    cx.stop_propagation();
                }
            }))
            // Enter on a list line starts the next item. Anywhere else the editor handles it.
            .capture_action(cx.listener(|this, _: &Enter, window, cx| {
                if this.vim_blocks(Some("enter"), window, cx) || this.continue_list(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .border_1()
            .border_color(c.border)
            .when(!editor.raw_mode, |d| d.child(toolbar))
            .child(
                Editor::new(&editor.body)
                    .appearance(false)
                    .font_family(mono.clone())
                    .text_size(px(13.))
                    .flex_1()
                    .min_h_0(),
            )
            .when(self.vim_on(), |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .border_t_1()
                        .border_color(c.border)
                        .font_family(mono.clone())
                        .text_size(px(11.))
                        .text_color(c.muted_fg)
                        .child(editor.vim.status()),
                )
            });

        let preview = div()
            .id("editor-preview")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .border_1()
            .border_color(c.border)
            .p_3()
            .child(TextView::markdown("preview", editor.body.read(cx).value()).selectable(true));

        let fields = div()
            .grid()
            .grid_cols(2)
            .gap_2()
            .child(field("Status", c).child(choice(
                "edit-status",
                Status::ALL.map(|s| (s, s.name())),
                editor.status,
                c,
                cx,
                |this, status, window, cx| {
                    if let Some(editor) = &mut this.editor {
                        editor.status = status;
                    }
                    this.editor_changed(window, cx);
                },
            )))
            .child(field("Priority", c).child(choice(
                "edit-priority",
                Priority::ALL.map(|p| (p, p.as_str())),
                editor.priority,
                c,
                cx,
                |this, priority, window, cx| {
                    if let Some(editor) = &mut this.editor {
                        editor.priority = priority;
                    }
                    this.editor_changed(window, cx);
                },
            )))
            .child(field("Assignee", c).child(Input::new(&editor.assignee).small()))
            .child(
                field("Due date", c).child(
                    DatePicker::new(&editor.due)
                        .small()
                        .cleanable(true)
                        .placeholder("No due date"),
                ),
            )
            .child(
                field("Labels", c)
                    .col_span(2)
                    .child(Input::new(&editor.labels).small()),
            );

        div()
            .id("sheet-backdrop")
            .absolute()
            .inset_0()
            .bg(black().opacity(0.3))
            .on_click(cx.listener(|this, _, window, cx| this.close_editor(window, cx)))
            .child(
                div()
                    .id("sheet")
                    .occlude()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(width)
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(c.bg)
                    .border_l_1()
                    .border_color(c.border)
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .font_family(mono.clone())
                                    .text_xs()
                                    .text_color(c.muted_fg)
                                    .child(format!("{project}/{}", editor.key.rel_path)),
                            )
                            .when(saving, |d| {
                                d.child(div().size_2().rounded_full().bg(c.link))
                            })
                            .child(
                                Button::new("sheet-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_editor(window, cx)
                                    })),
                            ),
                    )
                    .when(editor.deleted, |d| {
                        d.child(
                            self.banner("This file was deleted", c)
                                .child(listener_button("recreate", "Recreate").outline().on_click(
                                    cx.listener(|this, _, window, cx| this.recreate_card(window, cx)),
                                ))
                                .child(listener_button("deleted-close", "Close").ghost().on_click(
                                    cx.listener(|this, _, window, cx| this.close_editor(window, cx)),
                                )),
                        )
                    })
                    .when(editor.conflict && !editor.deleted, |d| {
                        d.child(
                            self.banner("Changed on disk", c)
                                .child(listener_button("keep-mine", "Keep mine").outline().on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.resolve_conflict(true, window, cx)
                                    }),
                                ))
                                .child(
                                    listener_button("load-theirs", "Load theirs").outline().on_click(
                                        cx.listener(|this, _, window, cx| {
                                            this.resolve_conflict(false, window, cx)
                                        }),
                                    ),
                                ),
                        )
                    })
                    .map(|d| match editor.raw_mode {
                        true => d
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(editor.base.data.title.clone()),
                            )
                            .child(self.banner(
                                "The frontmatter is not valid YAML. Fix it below: this is the whole file.",
                                c,
                            )),
                        false => d.child(Input::new(&editor.title).large()).child(fields),
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(tab_button("tab-markdown", "Markdown", EditorTab::Markdown, cx))
                            .child(tab_button("tab-preview", "Preview", EditorTab::Preview, cx))
                            .child(div().flex_1())
                            .child(
                                Button::new("expand")
                                    .ghost()
                                    .xsmall()
                                    .icon(icon(if expanded { "arrows-minimize" } else { "arrows-maximize" }))
                                    .label(if expanded { "Collapse" } else { "Expand" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.prefs.editor_expanded = !this.prefs.editor_expanded;
                                        this.prefs_changed(cx);
                                    })),
                            ),
                    )
                    .map(|d| match tab == EditorTab::Preview && !editor.raw_mode {
                        true => d.child(preview),
                        false => d.child(markdown),
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("copy-ai-prompt")
                                    .outline()
                                    .small()
                                    .icon(icon("robot"))
                                    .label("Copy AI prompt")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.copy_ai_prompt(window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(c.muted_fg)
                                    .child("Autosaves to the markdown file."),
                            )
                            .child(
                                Button::new("delete-card")
                                    .danger()
                                    .small()
                                    .icon(icon("trash"))
                                    .label("Delete")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(key) = this.open_card.clone() {
                                            this.confirm = Some(Confirm::Delete(key));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("raw-toggle")
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(c.muted_fg)
                            .cursor_pointer()
                            .child(icon(if editor.raw_open { "chevron-down" } else { "chevron-right" }).size_3())
                            .child("Raw markdown file")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(editor) = &mut this.editor {
                                    editor.raw_open = !editor.raw_open;
                                }
                                cx.notify();
                            })),
                    )
                    .when(editor.raw_open, |d| {
                        d.child(
                            div()
                                .id("raw-file")
                                .max_h(px(200.))
                                .overflow_y_scroll()
                                .border_1()
                                .border_color(c.border)
                                .bg(c.muted.opacity(0.4))
                                .p_2()
                                .font_family(mono)
                                .text_size(px(11.))
                                .child(editor.preview_file(cx)),
                        )
                    }),
            )
    }
}
