# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.

## Status

- M0 de-risk: spikes done, waiting on Safi for the open items below
- M1 to M8: not started

## Decisions and deviations

- The spec files live in `resources/`, where Safi put them. The build spec calls the folder `docs/spec/`. Not moved.
- The Electron source is read from the local clone at `../MD-Kanban` (commit `edebd29`).

## M0 open items (need Safi)

1. Headings in the editor cannot be bigger than body text. OK to amend FR-MDE-01 to bold and coloured only?
2. Wikilinks and `==highlight==` are done by rewriting the source before parsing, not by a plugin on text nodes. A highlight renders as one inline box, so a long one wraps as a unit. OK, or should mdk send gpui-kit a one-line patch adding `mark` to its inline tags?
3. Drag: one 10 s drag on macOS, and a run on Windows and Linux (`cargo run -p ui-spike -- dnd`, the frame numbers print when the drag ends).
4. Electron baseline: cold start, idle memory after 10 minutes and edit-to-screen time on the 169-card board.

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

### Preview spike: every element renders, two deviations

`spikes/ui-spike/src/preview.rs`, 3 unit tests and 1 headless render test. Try it: `cargo run -p ui-spike -- preview`. Checked by eye in the real window on macOS, dark theme.

Renders: a GitHub callout, an Obsidian callout that starts folded, a wikilink, a dimmed missing wikilink, `==highlight==`, inline and block maths through `ratex-gpui`, a Mermaid flowchart through `mermaid-rs-renderer` (labels included), a highlighted Rust block with a language label and Copy, a GFM table, a task list, a local image with a width, and a remote image shown as text and never fetched. "It costs $5 a month, or $50 a year" stays text. A formula that fails to parse shows its source in red. No fallback crate was needed for maths or Mermaid.

Deviations from the spec's plan:

- An inline `MarkdownPlugin` replaces a whole mdast node. It cannot take a `[[wikilink]]` or `==highlight==` out of the middle of a text node. So the source is rewritten before parsing: `[[card|label]]` becomes `[label](mdk-wiki:card)`, a missing card becomes `mdk-missing:`, `==x==` becomes `[x](mdk-hl:)`, and `![[img|300]]` becomes a normal image with the width in its title. Code and maths are skipped. `on_link_click` handles `mdk-wiki:`, and a plugin on link nodes draws the other two.
- `<mark>` does not work as a route for highlight. In markdown mode the kit pairs only the `b i u s` inline tags (`inline_html_mark` in gpui-base 0.7.1).
- A highlight is one inline box, so a long highlight wraps as a unit and not word by word.
- The code block "header" is the kit's `code_block_actions` slot, which sits at the top right of the block. A full-width header bar would need a custom code block plugin.

Not exercised: clicking a folded callout to open it. The toggle is wired (`window.use_keyed_state` plus `on_click`), and both the open and the folded state render, but no test clicks it.

Rough edges for M6: inline maths sits a little high and small, Mermaid uses the renderer's stock dark theme and not mdk's colours, and Mermaid renders on the UI thread on first sight.

### Dependency note for NFR-02

On the three desktop targets the only HTTP-related crate is `gpui-pre-http-client`, GPUI's client trait with no client behind it. `gpui-pre-reqwest` appears only for other targets (it comes from `gpui-kit-assets`). `deny.toml` in M1 has to check the three desktop targets, or a plain ban on reqwest fails.

### Editor headings check: no

Headings in the editor cannot render larger than body text. gpui-kit's editor highlights with GPUI's `HighlightStyle`, which has colour, weight, style, background, underline, strikethrough and fade, but no font size. Headings can be bold and coloured. FR-MDE-01 asks for bigger, so this needs Safi's OK before M4.

## M0 Electron baseline

Measured so far, from the existing build in `../MD-Kanban/release/` (built 10 Sep 2026):

- Download size: `MD Kanban-0.0.0-arm64.dmg` is 196 MB. The unpacked `.app` is 603 MB.

Not measured: cold start to a rendered board, idle memory after 10 minutes, and edit-to-screen time. They need the Electron app running on Safi's 169-card board.
