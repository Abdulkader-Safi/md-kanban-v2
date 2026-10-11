use gpui_kit::component::input::EditorState;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowHandle, WindowOptions,
    base::Root, px, size,
};
use ui_spike::vim::{Mode, VimSpike, bind_keys};

const TEXT: &str = "one\ntwo\nthree";

fn open(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<VimSpike>, Entity<EditorState>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        bind_keys(cx);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(800.), px(480.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| VimSpike::new(TEXT, window, cx)),
        )
        .unwrap();
        let state = view.read(cx).state.clone();
        (window.downcast::<Root>().unwrap(), view, state)
    })
}

/// Focus the editor, put the cursor at 0, press the keys one by one.
fn press(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
    state: &Entity<EditorState>,
    keys: &[&str],
) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
    })
    .unwrap();
    more(cx, handle, keys);
}

fn more(cx: &mut TestAppContext, handle: WindowHandle<Root>, keys: &[&str]) {
    for key in keys {
        cx.update_window(handle.into(), |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    }
}

fn value(cx: &mut TestAppContext, state: &Entity<EditorState>) -> String {
    cx.update(|cx| state.read(cx).value().to_string())
}

fn cursor(cx: &mut TestAppContext, state: &Entity<EditorState>) -> usize {
    cx.update(|cx| state.read(cx).cursor())
}

#[gpui_kit::test]
fn hjkl_move_the_cursor_and_insert_no_text(cx: &mut TestAppContext) {
    let (handle, _view, state) = open(cx);
    press(cx, handle, &state, &["l", "l"]);
    assert_eq!(cursor(cx, &state), 2);
    more(cx, handle, &["j"]);
    assert_eq!(cursor(cx, &state), 6, "j keeps the column");
    more(cx, handle, &["h", "k"]);
    assert_eq!(cursor(cx, &state), 1);
    assert_eq!(value(cx, &state), TEXT, "normal mode keys must not type");
}

#[gpui_kit::test]
fn i_types_and_escape_returns_to_normal_without_closing_the_sheet(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx);
    press(
        cx,
        handle,
        &state,
        &["i", "x", "y", "escape", "l", "escape", "escape"],
    );
    assert_eq!(value(cx, &state), "xyone\ntwo\nthree");
    cx.update(|cx| {
        assert_eq!(view.read(cx).mode, Mode::Normal);
        assert_eq!(view.read(cx).sheet_closed, 0, "Escape leaked to the sheet");
    });
}

#[gpui_kit::test]
fn dd_deletes_the_line_and_u_brings_it_back(cx: &mut TestAppContext) {
    let (handle, _view, state) = open(cx);
    press(cx, handle, &state, &["j", "d", "d"]);
    assert_eq!(value(cx, &state), "one\nthree");
    more(cx, handle, &["u"]);
    assert_eq!(value(cx, &state), TEXT);
}

#[gpui_kit::test]
fn visual_selection_then_d_deletes_it(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx);
    press(cx, handle, &state, &["v", "l"]);
    cx.update(|cx| assert_eq!(state.read(cx).selected_range(), 0..2));
    more(cx, handle, &["d"]);
    assert_eq!(value(cx, &state), "e\ntwo\nthree");
    cx.update(|cx| assert_eq!(view.read(cx).mode, Mode::Normal));
}

#[gpui_kit::test]
fn editor_cmd_shortcuts_can_be_blocked_in_normal_mode(cx: &mut TestAppContext) {
    let (handle, _view, state) = open(cx);
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    press(cx, handle, &state, &[select_all]);
    cx.update(|cx| assert_eq!(state.read(cx).selected_range(), 0..0));
    more(cx, handle, &["i", select_all]);
    cx.update(|cx| assert_eq!(state.read(cx).selected_range(), 0..TEXT.len()));
}

/// The control for the Escape test: with Vim off, Escape does reach the sheet.
#[gpui_kit::test]
fn with_vim_off_escape_closes_the_sheet_and_keys_type(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx);
    cx.update(|cx| view.update(cx, |v, _| v.vim_on = false));
    press(cx, handle, &state, &["h", "escape"]);
    assert_eq!(value(cx, &state), "hone\ntwo\nthree");
    cx.update(|cx| assert_eq!(view.read(cx).sheet_closed, 1));
}
