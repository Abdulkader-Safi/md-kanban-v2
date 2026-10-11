# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.

## Status

- M0 de-risk: in progress
- M1 to M8: not started

## Decisions and deviations

- The spec files live in `resources/`, where Safi put them. The build spec calls the folder `docs/spec/`. Not moved.
- The Electron source is read from the local clone at `../MD-Kanban` (commit `edebd29`).

## M0 results

Filled in as each spike finishes.

### File spike: passes on macOS, CI not run yet

`spikes/file-spike`, 6 tests, green on macOS (M1) three runs in a row, about 0.9 s total.

- External edit, rename-save (vim and git style) and our own atomic write all reach the watcher inside 500 ms with the 150 ms debounce.
- A 50-file burst arrives in at most 3 batches inside 500 ms.
- A card written into a subfolder created after the watcher started is seen (FR-LIVE-04).
- Atomic write replaces a file another handle has open, and leaves no temp file.
- The temp file is named `.<name>.mdk-tmp-<pid>`, hidden so a scan skips it. It exists only for the length of one write.

CI run 38143509379 passed on `macos-14`, `windows-latest` and `ubuntu-22.04`. Exit criterion met.

### Vim spike: passes on macOS

`spikes/ui-spike/src/vim.rs`, 6 headless tests (`cargo test -p ui-spike --test vim`). Try it: `cargo run -p ui-spike -- vim`.

- `h j k l`, `i`, `Esc`, `dd`, `u` and visual `v` + `d` work on an `EditorState`.
- The cursor and selection can be set from code: `EditorState::set_selected_range(range, cx)`, then `replace(text, window, cx)` to edit. The toolbar can use the same two calls.
- Finding: GPUI runs key bindings before key-down listeners. So `capture_key_down` alone does not stop Escape or Cmd shortcuts. Plain keys are taken with `capture_key_down`, and keys the editor has bound are taken with `capture_action` on the same parent element. Both are in the spike. This is not the "custom key context" fallback, so no fallback is taken.
- With Vim on, Escape never reaches the sheet around the editor. A control test shows it does reach the sheet with Vim off.
- `EditorState::undo` is private. `u` dispatches the kit's `Undo` action, which works.
- Not done in the spike: a block cursor in normal mode. The editor draws a bar. To look at in M5.

### Drag and drop spike: passes on macOS, Windows and Linux not tried

`spikes/ui-spike/src/dnd.rs`, 3 headless tests (`cargo test -p ui-spike --test dnd`). Try it: `cargo run -p ui-spike -- dnd`.

- Five columns of 200 variable-height cards. Each column is a virtualised `gpui::list`. No fallback needed.
- `on_drag`, `drag_over`, `on_drop` and `on_drag_move` give a card-face preview, column highlight and a drop slot (a teal line between cards).
- The drop index comes from `ListState::bounds_for_item` on the rendered cards, so it works with virtualisation.
- Auto-scroll at the column top and bottom, and at the board's left and right edges, uses gpui-base's `AutoScroll` timer.
- Frame gaps during a drag are printed when the drag ends. Two real drags in a debug build on the M1: 84 frames, worst 16.4 ms, and 234 frames, worst 16.7 ms. None over 33 ms. Both were shorter than the 10 s the exit criterion names.
- Open: a 10 s drag, and a run on Windows and Linux. Neither can be done headless.

### Editor headings check: no

Headings in the editor cannot render larger than body text. gpui-kit's editor highlights with GPUI's `HighlightStyle`, which has colour, weight, style, background, underline, strikethrough and fade, but no font size. Headings can be bold and coloured. FR-MDE-01 asks for bigger, so this needs Safi's OK before M4.

## M0 Electron baseline

Not measured yet.
