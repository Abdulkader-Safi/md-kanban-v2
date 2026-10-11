# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.

## Status

- M0 de-risk: spikes done. Open items settled by best practice on Safi's say-so (see decisions)
- M1 core crates: done. Tests green on macOS, Windows and Ubuntu, and `cargo-deny` passes (CI run 38145737412)
- M2 app shell and read-only board: built and tested headless. The side-by-side look check against the Electron screenshot is still open
- M3 moving and creating: built and tested headless, CI green on three OSes (run for `7aed244`). The cross-check in the running Electron app is open
- M4 card editor and markdown editor: built and tested headless
- M5 Vim mode: built and tested, table tests for every FR-VIM-02 command, CI green on three OSes (run for `5888e2a`). The manual check in the real window is open
- M6 preview: built and tested headless
- M7 and M8: not started

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

## M6 notes

Built, in `mdk-markdown`: `source` (plain text: the source rewrite, link and wikilink resolution, callout headers, the dollar guard, code fence titles) and `preview` (gpui-kit's `TextView` with mdk's plugins, as one `view()` function any GPUI app can call). In the app: the Preview tab with the Subtasks box above the rendered body.

The preview renders GitHub-flavoured markdown (the kit's own), plus: GitHub and Obsidian callouts in five looks with `-` and `+` folding; `$inline$` and `$$block$$` maths through `ratex-gpui`, with the dollar guard and red source for a bad formula; Mermaid through `mermaid-rs-renderer`, drawn on a background thread and cached, with the source and error for a bad diagram; a code block header with the language or `title="..."` and Copy; `==highlight==`; `%% comments %%` hidden; wikilinks (`[[card]]`, `[[card|label]]`, `[[card#Heading]]`) and relative links that open the card; dead links and unresolved wikilinks dimmed; `![alt](path)` and `![[image.png|300]]` read from the project folder only; remote images shown as a link and never fetched; raw HTML shown as text, with `<br>` as a line break.

Checked:

- `crates/mdk-markdown/tests/source.rs`, 10 test groups. Every case in Electron's `render.check.ts` that does not depend on HTML output is there with the same input: link resolution inside the project only, image paths, wikilinks by title, id, path and heading, embeds, highlights, the dollar guard, callout types and folding, code titles, and that script, iframe, style and event-handler HTML comes out escaped.
- `crates/mdk/tests/editor.rs`: one card holding every element is opened on the Preview tab and each custom element kind is seen to draw, including the error states. Looking at the preview writes nothing. Ticking a subtask in the Subtasks box changes that one checkbox in the file and the card face count.

Not checked: how any of it looks in both themes, clicking a wikilink or a folded callout in the real window, and text selection.

Decisions and deviations:

- Wikilinks, embeds and `==highlight==` are rewritten in the source before parsing (M0 finding). A highlight is one inline box.
- Raw HTML is escaped in the source so the kit's HTML support never sees it. `<br>` and autolinks are left alone.
- A code fence's title is set aside by the rewrite and looked up by the block's code when the header draws, because the kit's code block hook gives the language and the code but not the fence's info string.
- Electron's `{2}` line highlights and `[!code ++]` diff markers in code blocks are not built. The spec does not ask for them.
- Callout types map to Electron's five looks (note, tip, warning, danger, quote).
- Mermaid uses the renderer's stock dark and light themes, not mdk's colours. A panic inside the renderer is caught and shown as the diagram's error.
- Heading anchors and the outline from Electron's renderer are not built. Nothing in the spec uses them.
- Inline maths is drawn at the text size with no baseline shift, so it can sit a little high.

## M5 notes

Built: the `mdk-vim` crate, a state machine over a `VimBuffer` trait with no UI in it; an adapter that makes gpui-kit's editor a `VimBuffer`; the mode line under the editor; the sidebar's "Vim keys in editor" switch now does something.

Checked:

- `crates/mdk-vim/tests/vim.rs`, 11 test groups with about 150 keystroke cases on a plain string buffer: every motion, counts, `i a I A o O`, `x dd yy p P`, `u` and `Ctrl-r`, `d c y` with each motion and doubled, visual and visual-line `d y c x > <`, `/` with `n` and `N`, `:w` and `:q`, the status line, and text with wide characters.
- `crates/mdk/tests/editor.rs`, 5 tests in the real editor: normal-mode keys move and edit and are never typed as text; shifted keys (`$`, `G`, `A`) reach Vim; insert mode types, and Enter and Backspace work there; Escape never closes the card with Vim on; Enter, Backspace, Delete and Tab do nothing in normal mode; visual select and delete; search; `:q` closes and saves; with Vim off the editor is plain and Escape closes.

Not checked: Vim by hand in the real window. On macOS a plain key can reach the input method before GPUI's key-down listeners in some setups, which the headless harness cannot show.

Decisions:

- Plain keys are taken with `capture_key_down`. The keys the editor has bound (Escape, Enter, Backspace, Delete, Tab) are taken with `capture_action`, as found in M0.
- `u` and `Ctrl-r` dispatch the editor's own Undo and Redo, because its history is private. After an undo the cursor is where the editor's history leaves it, not where Vim would put it.
- A `cw` followed by typing is more than one undo step. Vim makes it one.
- The cursor in normal mode is the editor's bar, not a block.
- `>` and `<` shift by two spaces.
- Search is a plain, case-sensitive substring search that wraps.
- While fixing broken frontmatter in the raw file, Vim is off.

## M4 notes

Built: the card editor as a right-hand sheet (672 px, Expand makes it two-thirds of the window): path line, title, status and priority chips, assignee, due date picker, labels, Markdown and Preview tabs (the last one used is remembered), a 20-button formatting toolbar with tooltips, gpui-kit's editor with the markdown grammar, Cmd or Ctrl+B, I and E, list continuation on Enter, "Copy AI prompt" with Electron's exact text, Delete with a confirm (undoable), a collapsible raw file view, autosave 500 ms after the last change with a flush before switching cards or closing, the "Changed on disk" banner with Keep mine and Load theirs, the "This file was deleted" banner with Recreate and Close, and raw-file editing for cards with broken frontmatter.

New crate `mdk-markdown` with the toolbar commands. All 24 cases from Electron's `md-commands.check.ts` pass, plus cases for list continuation.

Checked by `crates/mdk/tests/editor.rs`, 16 tests in a headless window: opening and closing a card writes nothing; typing then closing at once still saves; autosave waits 500 ms; the title and the first heading stay in step in both directions; a field edit writes only its own keys and keeps unknown keys; a change on disk reloads a clean editor silently; a change on disk during unsaved typing shows the banner and writes nothing until answered; Keep mine puts my body on top of their file; Load theirs drops mine; a deleted file can be recreated; the toolbar toggles marks and is one undo step; broken frontmatter is fixed in the raw file and the form comes back; switching cards saves the one being left and never writes to the wrong file; delete asks first and undo restores; Escape closes and saves.

Not checked: how the sheet looks, the editor's syntax colours, find in card (Cmd+F, which is the kit's own search), IME input, and the date picker by hand.

Decisions and deviations:

- The form is compared with the card as loaded, and only the fields that differ are written, on top of the file as it is on disk at that moment. "Keep mine" therefore keeps my changed fields and their other fields. The spec says "overwrite with your version". Field-level is safer for an agent's edit to another field.
- The title and the first heading are kept in step in both directions. Electron's editor sends the body with every save, so its title field does not change the heading.
- Headings in the editor are bold and coloured, not bigger (M0 finding, FR-MDE-01 amended).
- Enter continuing lists, checklists and quotes is mdk's own (`commands::continue_list`). gpui-kit's editor does not do it.
- Toolbar edits change only the part of the text that differs, so each is one step in the editor's own undo.
- The Preview tab is gpui-kit's plain markdown view for now. The Subtasks box, maths, Mermaid, callouts and wikilinks are M6.
- A card whose file is too large or not UTF-8 does not open. A toast says why.
- While the editor is open, Cmd+Z in the body is the editor's text undo. Board undo (Cmd+Z on the board) can undo an autosave as one "Edit" entry.
- The sheet, like the dialogs, is a hand-built overlay, not gpui-kit's `Sheet`.

## M3 notes

Built: drag and drop between and within columns with a drop slot, column highlight and edge auto-scroll; the "⋯" move menu; keyboard moving (arrows move the focus, Space picks up, arrows move the card, Space drops, Esc puts it back); undo and redo of move, create, delete and archive on Cmd or Ctrl+Z and Shift+Z; a "Moved to X" toast with Undo for 5 seconds; the New card dialog with quick-add hint, the three templates and a date picker; archive with a confirm that states the count.

Checked by tests (74 in the workspace):

- `crates/mdk/tests/board_writes.rs`, 13 tests on real files: a move writes only `status`, `order` and `modified`; unknown keys survive; an agent's unseen edit survives a move; colliding orders renumber the column; a broken card cannot be moved; undo and redo restore exact bytes; undo is refused when the file changed since; undo of a delete restores a CRLF file with a BOM byte for byte; archive renames without changing content.
- `crates/mdk/tests/app.rs`, 14 tests in a headless window: a real drag rewrites the file; dropping a card where it is writes nothing; a broken card's drag is refused with the toast; the keyboard move and its Escape; `N` then typing then Enter creates a card from quick-add; Escape closes the dialog; archive asks first.

Not checked: the look of any of it (screen locked), drag smoothness on the real board, the move menu's placement on screen, the date picker by hand, and Electron opening mdk-written files in the running Electron app. The golden tests do show mdk's output matches Electron's writer byte for byte.

Decisions:

- A drop's index is taken from the cards the column shows, and so is the new `order`. Electron takes neighbours from the whole project, which gives odd results when a filter is on.
- When orders collide, the visible column is renumbered 0, 10, 20 and each renumbered file is written. Electron renumbers in memory only.
- Archive is a pure rename. Electron rewrites the file and bumps `modified`. A rename keeps `git status` showing renames, as FR-BOARD-05 asks.
- A new card's id is its file stem, including a `-2` suffix, so ids stay unique in a project.
- Keyboard focus is one "focused card" owned by the board area, moved with the arrow keys and opened with Enter. Electron uses the browser's Tab order. Plain keys (N, Space, arrows, Enter) are bound to a "Board" key context so they never fire while typing in a dialog.
- Esc during a keyboard move writes the card's old status and order back as one more edit. It is on the undo stack.
- Toasts, the confirm dialog and the New card dialog are hand-built overlays, not gpui-kit's notification and dialog. Status and priority are chip rows, not selects.
- Own-write detection keeps the previous save of a file for one second as well as the latest. CI on Linux showed the watcher can read a file between two quick saves.
- The file watcher is switched off in headless tests except the one live-update test. GPUI's test scheduler does not allow wake-ups from other threads.

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
