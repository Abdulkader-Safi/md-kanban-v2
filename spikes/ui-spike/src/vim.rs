//! Vim spike: prove that keys can be taken before gpui-kit's editor sees them,
//! and that the cursor and selection can be set from code.
//!
//! Finding: GPUI runs key *bindings* before key-down listeners. So plain keys
//! (`h`, `d`, `i`) are caught with `capture_key_down`, and keys the editor has
//! bound (Escape, Cmd shortcuts) are caught with `capture_action`.

use gpui_kit::component::input::{Editor, EditorState, Escape, SelectAll, Undo};
use gpui_kit::*;

actions!(vim_spike, [CloseSheet]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
}

pub struct VimSpike {
    pub state: Entity<EditorState>,
    pub mode: Mode,
    /// The sidebar switch. Off means the editor behaves as if Vim did not exist.
    pub vim_on: bool,
    /// How many times Escape reached the "sheet" around the editor. Must stay 0.
    pub sheet_closed: usize,
    pending_d: bool,
    anchor: usize,
    head: usize,
}

fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(text: &str, pos: usize) -> usize {
    text[pos..].find('\n').map_or(text.len(), |i| pos + i)
}

/// `h j k l` on byte offsets. ASCII only, which is enough for a spike.
fn motion(text: &str, pos: usize, key: &str) -> Option<usize> {
    let (ls, le) = (line_start(text, pos), line_end(text, pos));
    let col = pos - ls;
    Some(match key {
        "h" => pos.saturating_sub(1).max(ls),
        "l" => (pos + 1).min(le.saturating_sub(1).max(ls)),
        "j" if le < text.len() => {
            let nle = line_end(text, le + 1);
            (le + 1 + col).min(nle)
        }
        "k" if ls > 0 => {
            let pls = line_start(text, ls - 1);
            (pls + col).min(ls - 1)
        }
        "j" | "k" => pos,
        _ => return None,
    })
}

impl VimSpike {
    pub fn new(text: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = text.to_string();
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .soft_wrap(true)
                .default_value(text)
        });
        Self {
            state,
            mode: Mode::Normal,
            vim_on: true,
            sheet_closed: 0,
            pending_d: false,
            anchor: 0,
            head: 0,
        }
    }

    fn select(&self, range: std::ops::Range<usize>, cx: &mut Context<Self>) {
        self.state
            .update(cx, |s, cx| s.set_selected_range(range, cx));
    }

    fn delete(&self, range: std::ops::Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.set_selected_range(range, cx);
            s.replace("", window, cx);
        });
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = ev.keystroke.modifiers;
        if !self.vim_on || self.mode == Mode::Insert || m.platform || m.control || m.alt {
            return;
        }
        // Normal and visual mode own every plain key: nothing reaches the editor as text.
        cx.stop_propagation();
        let key = ev.keystroke.key.as_str();
        let text = self.state.read(cx).value().to_string();
        let pending_d = std::mem::take(&mut self.pending_d);

        if self.mode == Mode::Visual {
            if let Some(pos) = motion(&text, self.head, key) {
                self.head = pos;
            } else if key == "d" {
                let (a, b) = (self.anchor.min(self.head), self.anchor.max(self.head));
                self.delete(a..(b + 1).min(text.len()), window, cx);
                self.mode = Mode::Normal;
                cx.notify();
                return;
            }
            let (a, b) = (self.anchor.min(self.head), self.anchor.max(self.head));
            self.select(a..(b + 1).min(text.len()), cx);
            return;
        }

        let pos = self.state.read(cx).cursor();
        if let Some(pos) = motion(&text, pos, key) {
            self.select(pos..pos, cx);
            return;
        }
        match key {
            "i" => self.mode = Mode::Insert,
            "v" => {
                self.mode = Mode::Visual;
                (self.anchor, self.head) = (pos, pos);
                self.select(pos..(pos + 1).min(text.len()), cx);
            }
            "d" if pending_d => {
                let (ls, le) = (line_start(&text, pos), line_end(&text, pos));
                self.delete(ls..(le + 1).min(text.len()), window, cx);
            }
            "d" => self.pending_d = true,
            "u" => window.dispatch_action(Box::new(Undo), cx),
            _ => {}
        }
        cx.notify();
    }

    fn on_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if !self.vim_on {
            return;
        }
        // Escape never reaches the editor or the sheet while Vim is on (FR-VIM-03).
        cx.stop_propagation();
        if self.mode == Mode::Visual {
            self.select(self.head..self.head, cx);
        }
        self.mode = Mode::Normal;
        self.pending_d = false;
        cx.notify();
    }
}

impl Render for VimSpike {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sheet")
            .key_context("Sheet")
            .on_action(cx.listener(|this, _: &CloseSheet, _, cx| {
                this.sheet_closed += 1;
                cx.notify();
            }))
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_2()
            .child(
                div()
                    .id("vim")
                    .flex_1()
                    .capture_key_down(cx.listener(Self::on_key))
                    .capture_action(cx.listener(Self::on_escape))
                    .capture_action(cx.listener(|this, _: &SelectAll, _, cx| {
                        if this.vim_on && this.mode != Mode::Insert {
                            cx.stop_propagation();
                        }
                    }))
                    .child(Editor::new(&self.state).size_full()),
            )
            .child(format!("-- {:?} --", self.mode).to_uppercase())
    }
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseSheet, Some("Sheet"))]);
}
