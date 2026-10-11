# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.

## Status

- M0 de-risk: spikes done. Open items settled by best practice on Safi's say-so (see decisions)
- M1 core crates: done. Tests green on macOS, Windows and Ubuntu, and `cargo-deny` passes (CI run 38145737412)
- M2 app shell and read-only board: built and tested headless. The side-by-side look check against the Electron screenshot is still open
- M3 to M8: not started

## Decisions and deviations

- The spec files live in `resources/`, where Safi put them. The build spec calls the folder `docs/spec/`. Not moved.
- The Electron source is read from the local clone at `../MD-Kanban` (commit `edebd29`).
- The repo is `github.com/Abdulkader-Safi/md-kanban-v2`. The spec names it `mdk`.
- Safi, 2026-10-11: settle open items by best practice and keep building, since only he uses the tool for now. So:
  - Editor headings are bold and coloured, not bigger. FR-MDE-01 is amended.
  - Wikilinks and `==highlight==` use the source rewrite from the preview spike. A long highlight wraps as a unit. Worth a gpui-kit patch later.
  - The 10 s drag, the Windows and Linux drag runs, and the Electron cold start, memory and latency numbers move to the M8 benchmark and release checklist.
- `mdk-format` reads frontmatter with a port of Electron's own line parser, not with `serde-saphyr`. Electron does not use a YAML parser, and only a port gives the same field values on every fixture. `serde-saphyr` still checks the frontmatter is valid YAML and sets `parse_error` (FR-FILE-05).
- CRLF files: Electron's parser reads no fields from a CRLF file (its regex stops at `\r`). mdk normalises line endings before parsing and writes CRLF back (FR-FILE-08).
- Frontmatter comment lines are kept on write, after the nine known keys, along with the unknown keys.
- Due labels count whole days between dates. Electron rounds milliseconds from the current time, so its label changes during the day.
- The watcher uses event paths only, never event kinds. On macOS, FSEvents reports a mix of everything that happened to a path lately, and does not report the old name of a renamed file at all. The store keeps the set of card paths it knows, checks each reported path on disk, and sweeps for vanished cards when an unknown path shows up.
- `StoreEvent` has no `Renamed`. A rename arrives as `Removed` plus `Changed`, which is all the Board needs.
- The store does not depend on `mdk-format`. `Store::update` takes a closure, and the Board passes `mdk_format::apply_patch` to it. Both crates stay usable on their own (NFR-09).
- Own-write suppression lives in the store, by content hash, as the spec says. The burst rule (more than 200 paths in one debounced batch becomes one `Rescan`) is checked on the batch function directly, because 250 real writes arrive as several batches.
- The spikes stay in `spikes/` as reference until the milestones that replace them. They are workspace members but not default members.

## M2 notes

Run it: `cargo run -p mdk`. It keeps its settings in the OS config folder, or in `MDK_CONFIG_DIR`.

Built: window with saved size and position, dark, light and system themes from `crates/mdk/assets/theme/mdk.json`, Geist font and Tabler icons bundled, header, sidebar (connect, projects with open counts, rescan, remove, Locate folder, work folders, auto-refresh and Vim switches, "checked" time), five virtualised columns, card faces, welcome and "No cards match" states, a read-only card sheet, background scans, live updates from the watcher, and `config.json`.

Checked by tests (`cargo test -p mdk`, 19 tests): the board logic, the config file, and the real root view in a headless window: loading a saved project into columns, connecting from the welcome state, saving and restoring selection and toggles, removing a project, a card moving when another program edits its file, the card sheet opening and closing on Escape, and a second launch signalling the first.

Not checked yet:

- How it looks. The screen was locked while this was built, so no window capture was possible. The comparison with `references/electron-board-screenshot.png` and Safi's sign-off on it are open.
- Safi's real `clinicos` folder. The `MD-Kanban` task folder was only loaded in a manual run, not compared card by card with Electron.
- FR-BOARD-06 (1,000 cards without slow frames). The columns are virtualised as in the drag spike, but it is not measured.

Deviations and stand-ins:

- The header's Search button and the welcome state's "Create sample project" are shown disabled. The palette is M7 and the sample project is M8.
- "New card", "+", "+ Add card" and "Create first card" show a one-line notice until the dialog lands in M3.
- A notice line above the board stands in for toasts until M3.
- The card sheet is hand-built and read-only. gpui-kit's `Sheet` and the editor replace it in M4.
- Single instance works on macOS and Linux through a Unix socket in the config folder. Windows has no check yet (M8).
- Fonts: Electron uses Roboto for body text and Geist for headings. mdk uses Geist throughout, as the spec says. The files are Fontsource's Latin subset, converted from WOFF. Other scripts fall back to a system font.
- The card key is hashed into each column list's row identity, so a changed card is re-measured and the list keeps its scroll position.
- Card file writes will run on the UI thread when they arrive in M3. Scans already run in the background.
- No log file yet. Watcher errors are dropped, and the next rescan recovers.

## M1 notes

- `mdk-format`: 46 golden fixtures (35 real cards from the Electron repo's `tasks/` and `examples/`, 11 odd cases) parse to the same fields as Electron's `parseTaskFile` and write byte for byte like `stringifyTaskFile`. Generated by `crates/mdk-format/tests/fixtures/generate.ts` under bun.
- `mdk-store`: scan rules, path guard including symlinks, atomic write with a simulated crash, re-read-merge-write, own-write suppression, external edit and rename inside 500 ms, a 50-file burst in at most 3 batches, new subfolders, a removed root.
- `deny.toml` is written but `cargo-deny` is not installed locally. It runs in CI only.
- `cargo-deny` checks the three desktop targets. Today it only sees the crates in the lock file, which includes the spikes' GPUI tree.

## M0 open items (settled, see decisions)

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
