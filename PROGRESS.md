# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.


## First Windows run (2026-10-28)

- Release builds use the Windows GUI subsystem, so no console window opens beside mdk. Closing that console used to kill the app. Debug builds keep the console. Logs were already in `mdk.log`.
- `crates/mdk/build.rs` embeds `icon.ico` in `mdk.exe` as icon resource 1, the ID GPUI loads for the window and taskbar. Explorer and the NSIS shortcuts read the same resource.
- Open: SmartScreen still says "Windows protected your PC". Only an Authenticode signature fixes that. Not signing for now.

## Fixes from daily use (2026-10-18)

- A click inside the card sheet started a text selection that followed the pointer. The sheet, dialogs and three menu buttons stopped click propagation, which also swallowed the mouse up that ends a gpui-kit window selection. They use `occlude()` now. Rule: never `stop_propagation` in an `on_click` to keep a click off a parent. Use `occlude()`.
- The sidebar (180 to 420 px) and the card sheet (420 px up to leaving the board 320 px) resize by dragging their edge. Widths are saved in `config.json`.
- Subtasks wrap, and their inline code renders as code. Card excerpts are plain text.

## App icon and releases (2026-10-14)

- The app icon is the sidebar's kanban mark in lime on an ink rounded tile. `scripts/app-icon.sh` rebuilds the `.icns`, `.ico` and `.png` from `app-icon.svg`.
- The macOS bundle id is `com.abdulkadersafi.mdk`. The bundle gets an ad-hoc signature (`signing-identity = "-"`), so a downloaded copy shows "Open Anyway" in Privacy & Security instead of "damaged". Real signing and notarisation need a paid Apple Developer account.
- The release build is arm64 only, because `macos-14` runners are Apple Silicon. Add an Intel or universal build if someone asks for one.
- Releases: push a `v*` tag and `release.yml` attaches the dmg, the Windows installer and the AppImage.

## Redesign from Figma (2026-10-12)

The app now follows the Figma file `uuKuQLmZPhiDZnq0gpoDcZ` (pages Cover, Foundation, Design). Same features and flow, new look.

- Theme: near-black surfaces, one lime accent (`#c8f031`), a warm paper light theme. All colours are in `crates/mdk/assets/theme/mdk.json`. Status and priority colours are in `crates/mdk/src/theme.rs`.
- Fonts: Schibsted Grotesk for UI text and JetBrains Mono for paths, counts and the editor. Both are OFL 1.1 and bundled. Geist is gone. This departs from the build spec, which names Geist and a system monospace.
- Layout: the sidebar runs the full height and holds the brand. A top bar over the board holds the path, search, the Board or List switch, rescan, theme and New card. The filter bar keeps priority, due, assignee, labels and saved views.
- Cards: outlined priority badge and file name on top, then title, excerpt, labels, and a footer with due date, subtasks, work folder and the assignee's initial.
- Editor: icon buttons in the header (copy AI prompt, expand, delete, close), field rows with mono captions, underlined tabs, a Vim switch next to the tabs, and a mode chip in the status line.
- Dialogs share one shell in `views/mod.rs`: header, body, sunken footer.
- The focus ring uses the accent text colour, which is dark green in the light theme, so it keeps 3:1 on white.

Not done: no screenshot check of the new UI. The screen was locked, so window capture failed. The 142 tests pass, including the contrast test and the headless render tests.

## Status

- M0 de-risk: spikes done. Open items settled by best practice on Safi's say-so (see decisions)
- M1 core crates: done. Tests green on macOS, Windows and Ubuntu, and `cargo-deny` passes (CI run 38145737412)
- M2 app shell and read-only board: built and tested headless. The side-by-side look check against the Electron screenshot is still open
- M3 moving and creating: built and tested headless, CI green on three OSes (run for `7aed244`). The cross-check in the running Electron app is open
- M4 card editor and markdown editor: built and tested headless
- M5 Vim mode: built and tested, table tests for every FR-VIM-02 command, CI green on three OSes (run for `5888e2a`). The manual check in the real window is open
- M6 preview: built and tested headless
- M7 filters, views, list and palette: built and tested headless
- M8 first run, agent contract, packaging: built. The release itself, the cross-OS checklist and most of the speed numbers are open

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

## First look, 12 October 2026

The screen was unlocked, so the release build was run on the Electron repo's task folders and on the sample project, and its window captured. Seen: the board in both themes, the card editor's Markdown and Preview tabs, the New card dialog, the command palette, the list view and the settings dialog. The layout matches the Electron screenshot's structure: header, 240 px sidebar, filter bar, 288 px columns, card faces.

Three defects showed up, none of which a headless test could see. All are fixed:

- Long excerpts were cut off at the card's edge. They now end in an ellipsis.
- In the editor, link text, wikilinks and callout types were dark navy on the dark background. The theme file had no syntax colours, so the editor kept gpui-kit's light ones. `mdk.json` now carries a `highlight` section per theme, with Electron's `--md-*` colours.
- The list view's columns did not line up, because each row was only as wide as its content.

`MDK_SHOW` opens screens at launch for checks like this: `MDK_SHOW="card=writing/x.md;preview" cargo run -p mdk`.

A second pass, with a wider window and a card holding only the preview's lower half, showed all five columns with Done's "archive" action, and the code block header, maths, a bad formula in red, the Mermaid flowchart, a table and escaped HTML all rendering in the real app. It also showed a fourth defect, now fixed: a column could start scrolled partway down when a second project's cards sorted above the first one's.

Still not seen: sideways scrolling, the native menu, the dropdowns and the move menu open, and anything that needs a click. Inline code in the preview has a teal background (it takes the theme's accent colour), which is louder than Electron's.

## Measurements, 12 October 2026

With `scripts/bench.sh` on the M1, mdk 0.1.0 release build against the Electron build in `../MD-Kanban/release`:

- Download: `dist/mdk_0.1.0_aarch64.dmg` is 11.0 MB. Electron's dmg is 196 MB.
- App on disk: 28 MB against 603 MB.
- Idle memory after 600 s, all of each app's processes, both running at once: 67 MB against 250 MB.
- Launch to first window, median of 5 warm launches: 515 ms (497, 530, 515, 418, 656) against 521 ms (572, 509, 486, 521, plus a first launch of 3,867 ms). mdk's first launch after a build was 1,518 ms.

Limits of these numbers: the Electron desktop build has no saved project list on this machine and `clinicos` is not here, so memory and launch were taken with no project connected in either app. Launch to first window does not show when the board is drawn. On that measure the apps tie, so NFR-01's "beat Electron on cold start" is not shown. Edit-to-screen time is not measured for Electron.

## What is left

Everything in the spec is built. What is missing is checking. In order of how much it matters:

1. Look at it next to Electron. The main screens have been captured and four defects fixed (see "First look"), but nobody has compared it card by card with `references/electron-board-screenshot.png` on the same folders. Run `cargo run --release -p mdk`.
2. Use it by hand on macOS: drag, the menus, the date picker, clicking links and folded callouts in the preview, Vim keys, find in card, IME input.
3. Open the same folder in the Electron app and mdk at once (NFR-08).
4. Run `scripts/bench.sh` for both apps on the real 169-card board. The README has numbers for an empty board only (NFR-01).
5. Try it on Windows and Linux, then push a `v0.1.0` tag to make the release (FR-PKG-01, NFR-05). Pushing the tag publishes a public release, so that is Safi's call.
6. Try single instance and reduce motion by hand. Both are built: single instance is tested on macOS and Linux and compiles on Windows, and reduce motion is gpui-kit's own reading of the OS setting, re-read when the window is activated.

## Acceptance checklist

66 of 98 boxes are ticked. A tick means an automated test covers it. An open box says what is missing. Nothing is ticked on the strength of the code looking right.

### POC (proves the core idea, end of M3)
- [ ] **FR-PROJ-01** Connect folder opens the native picker, and the chosen folder shows as a selected project with its cards.
  - Open: Connecting a folder is tested. The native picker itself is not: no window.
- [x] **FR-FILE-02** Every Electron fixture card parses to the same fields as Electron's parser.
  - Checked: golden test, 46 fixtures
- [x] **FR-FILE-03** Every fixture round-trips byte for byte except `modified`, and Electron reads mdk-written files identically.
  - Checked: golden test: mdk's writer output equals Electron's byte for byte. Not yet opened in the running Electron app.
- [x] **FR-FILE-04** A card with `sprint: 4` and `agent: "claude"` keeps both keys, verbatim, after a drag.
  - Checked: format and board tests
- [x] **FR-LIVE-01** A script changing a card's `status` moves it on the board within 500 ms. A 50-file git checkout gives one refresh with no freeze.
  - Checked: store tests on three OSes and a headless app test. "No freeze" on a 50-file checkout is not observed by eye.
- [x] **FR-MOVE-01** Dragging a card between two cards in another column lands it there, and its file gets the new status and an in-between order.
  - Checked: headless drag test and board test
- [x] **FR-AGENT-03** An agent edits a closed card's body, the user drags it, and the file has both the new status and the agent's body.
  - Checked: board and editor tests
- [x] **FR-AGENT-02** After a session of creating, moving and deleting, `git status` shows only card `.md` changes.
  - Checked: store and board tests list the folder after a session

### MVP (shippable v1, end of M8)
Projects and folders
- [x] **FR-PROJ-02** Two connected folders survive a restart.
  - Checked: app test
- [x] **FR-PROJ-03** Project counts show cards that are not done (37 cards with 10 done shows 27).
  - Checked: board test
- [x] **FR-PROJ-04** Project selection filters the board and survives a restart.
  - Checked: app test
- [x] **FR-PROJ-05** Work folders list and filter correctly, including across All projects.
  - Checked: board test
- [x] **FR-PROJ-06** `archive/` cards are hidden unless the archive work folder is selected.
  - Checked: board and app tests
- [x] **FR-PROJ-07** Remove project leaves files on disk unchanged.
  - Checked: app test
- [x] **FR-PROJ-08** Rescan all re-reads every project.
  - Checked: every app test starts with a rescan of all projects
- [x] **FR-PROJ-09** A renamed or moved folder shows "Folder not found" with Locate folder, and relocating restores it.
  - Checked: board test. The sidebar's "Folder not found" row is not seen by eye.

Card files
- [x] **FR-FILE-01** Scanning follows the depth 4 and skip rules, and every `.md` is a card.
  - Checked: store test
- [x] **FR-FILE-05** Broken YAML shows a card with a warning. Dragging it is blocked with a toast, and the file is never rewritten by board actions.
  - Checked: board, app and editor tests
- [x] **FR-FILE-06** New files are `<slug>-YYYY-MM-DD.md` in the right folder, with `-2` on a clash.
  - Checked: board test
- [x] **FR-FILE-07** Writes are atomic (temp file plus rename). A test kills the write midway and the original is intact.
  - Checked: store test with a simulated crash before the rename
- [x] **FR-FILE-08** UTF-8 with BOM is read. CRLF files stay CRLF after a save.
  - Checked: format tests
- [x] **FR-FILE-09** Paths that leave the root (`..`, symlinks out) are refused. Non-image files can't be read as images.
  - Checked: store tests, including symlinks

Board and cards
- [x] **FR-BOARD-01** Five columns with Electron's names and colours.
  - Checked: names and colours are constants in `mdk-format`. The look is not checked.
- [ ] **FR-BOARD-02** Column header shows dot, name, count and "+", plus "+ Add card" at the bottom.
  - Open: Seen in the real window: dot, name, count, "+", "archive" on Done, and "+ Add card". The buttons have not been clicked.
- [x] **FR-BOARD-03** Cards are sorted by `order`.
  - Checked: board test
- [ ] **FR-BOARD-04** Horizontal board scroll, per-column vertical scroll, and no vertical overscroll of the board.
  - Open: Built (sideways board scroll, one list per column). Scrolling is not checked by hand.
- [x] **FR-BOARD-05** Archive moves all visible done cards into `archive/` after a confirm with the count.
  - Checked: board and app tests
- [ ] **FR-BOARD-06** A 1,000-card project scrolls and drags without frames over 33 ms.
  - Open: Frame times on 1,000 cards are not measured. Scan (31 ms) and filtering (0.3 ms) are, in a release build.
- [ ] **FR-CARD-01** The card face matches the Electron screenshot for the same files.
  - Open: Seen in the real window on the Electron repo's own cards, and it matches the screenshot's structure. Not compared card by card.
- [x] **FR-CARD-02** Clicking a card opens the editor sheet.
  - Checked: app test
- [ ] **FR-CARD-03** The "⋯" menu moves the card to the end of the chosen column, with the current column disabled.
  - Open: Moving to the end of a column is tested. The menu itself has not been clicked.
- [ ] **FR-CARD-04** Cards with broken frontmatter show a warning with the error on hover.
  - Open: The warning icon and tooltip are built. Hover is not checked.

Moving
- [x] **FR-MOVE-02** A move changes only `status`, `order` and `modified`, and colliding orders renumber the column.
  - Checked: board tests
- [x] **FR-MOVE-03** The "Moved to X" toast's Undo within 5 s restores the status and order.
  - Checked: app test for the toast and its Undo. The 5-second timeout is not timed.
- [x] **FR-MOVE-04** Keyboard moving with Space, arrows, Space and Esc works, Esc restores the original place, and the hint shows.
  - Checked: app test
- [x] **FR-MOVE-05** ⌘Z and ⌘⇧Z undo and redo a move, create, delete and archive. Undo of delete restores the exact bytes.
  - Checked: board and app tests

Creating
- [x] **FR-NEW-01** New card opens from the header, `N`, column "+" and "+ Add card", with the right default status and work folder.
  - Checked: `N` is tested. The header button, "+" and "+ Add card" call the same function.
- [x] **FR-NEW-02** `Fix login p1 #backend @safi due:friday` on 2026-10-11 gives "Fix login", critical, #backend, @safi, due 2026-10-16, and the hint line shows it.
  - Checked: format test for the Sunday example, app test for the dialog
- [x] **FR-NEW-03** The bug, feature and chore templates fill priority, labels and body like Electron.
  - Checked: format test. The three buttons have not been clicked.
- [x] **FR-NEW-04** The created card's file exists and the card shows at once.
  - Checked: app test
- [ ] **FR-NEW-05** The dialog closes on Cancel, Esc and a backdrop press-and-release, but not when drag-selecting out of it.
  - Open: Esc is tested. Cancel, the backdrop click and the drag-select case are not.

Card editor
- [ ] **FR-EDIT-01** The sheet shows path, title, field grid with date picker, and labels. Expand works.
  - Open: Seen in the real window: path, title, the field grid with the date picker, labels. Expand is not tested.
- [x] **FR-EDIT-02** Typing then closing within 100 ms saves to the file.
  - Checked: editor test
- [x] **FR-EDIT-03** Renaming the title updates the body's first `# ` heading.
  - Checked: editor test, both directions
- [ ] **FR-EDIT-04** Markdown and Preview tabs, with the last tab remembered.
  - Open: Built, and the tab is in the config file. Restoring it is not tested.
- [x] **FR-EDIT-05** Copy AI prompt puts Electron's prompt text on the clipboard and shows "Copied".
  - Checked: the prompt text is tested. The clipboard and the toast are not.
- [x] **FR-EDIT-06** Delete confirms, removes the file and closes, and is undoable.
  - Checked: editor test
- [ ] **FR-EDIT-07** The raw file view shows the exact text that will be written.
  - Open: Built. Not tested.
- [x] **FR-EDIT-08** With unsaved typing, an external change shows "Changed on disk" with Keep mine and Load theirs. When clean, it reloads within 500 ms.
  - Checked: editor tests
- [x] **FR-EDIT-09** An external delete shows "This file was deleted" with Recreate and Close.
  - Checked: editor test
- [x] **FR-EDIT-10** The sheet closes on X, Esc (not in Vim mode) and backdrop press-and-release.
  - Checked: Esc is tested, with Vim off and on. X and the backdrop are not.

Markdown editor
- [ ] **FR-MDE-01** Markdown highlighting, wrapping, active line, selection matches and placeholder, in both themes.
  - Open: Seen in the real window after the syntax colours were fixed: bold headings, teal links, dimmed markup. Headings are bold, not bigger (amended in M0). Active line and selection matches are not checked.
- [x] **FR-MDE-02** All 20 toolbar actions behave like Electron's, and the ported `md-commands.check.ts` cases pass.
  - Checked: all 24 ported cases pass, plus editor tests
- [ ] **FR-MDE-03** ⌘B, ⌘I, ⌘E, ⌘F, undo and redo, Tab and Shift-Tab, and list continuation on Enter.
  - Open: Bold, undo and list continuation are tested. Italic and code share bold's path. Find, Tab and Shift-Tab are the kit's own and are not tested here.
- [ ] **FR-MDE-04** Selection, clipboard, word and line navigation, and IME input (tested with a Japanese or Arabic input method).
  - Open: The kit's editor. IME input is not tested.

Vim
- [x] **FR-VIM-01** The sidebar Vim switch toggles Vim in the editor and persists.
  - Checked: app tests
- [x] **FR-VIM-02** Every command in the FR-VIM-02 list passes the table test, and the mode line shows the mode.
  - Checked: table test, about 150 cases
- [x] **FR-VIM-03** Esc in Vim mode never closes the sheet.
  - Checked: editor test

Preview
- [ ] **FR-PREV-01** GFM renders: tables (scroll when wide), task lists, footnotes, strikethrough, blockquotes, rules and code.
  - Open: Seen in the real window: headings, lists, task lists, a table, inline code, a code block. Footnotes and wide tables are not checked.
- [x] **FR-PREV-02** The Subtasks box ticks the right `- [ ]` line in the file, and the card face count updates.
  - Checked: editor test
- [x] **FR-PREV-03** Code blocks are highlighted, with a language or title header and a working Copy.
  - Checked: the header is seen to draw, and titles are tested. Copy has not been clicked.
- [x] **FR-PREV-04** `$x^2$` and `$$\int$$` render as maths, "costs $5 a month, or $50" stays text, and a bad formula shows red source.
  - Checked: guard tested, a formula and a bad formula are seen to draw
- [ ] **FR-PREV-05** A Mermaid flowchart renders in theme colours, and a broken one shows source and error.
  - Open: A flowchart and a broken diagram's error are seen to draw. It uses the renderer's stock themes, not mdk's colours.
- [ ] **FR-PREV-06** GitHub and Obsidian callouts render in five looks, and `-` and `+` folding works.
  - Open: Types, titles and fold markers are tested, and open and folded callouts draw. Clicking to fold is not tested.
- [x] **FR-PREV-07** `==highlight==` renders, and `%% comments %%` are hidden in the preview and on the card face but kept in the file.
  - Checked: format and source tests, and the highlight is seen to draw
- [ ] **FR-PREV-08** External links open in the browser, card links and wikilinks open the card, links outside the project are plain text, and unresolved wikilinks are dimmed.
  - Open: Resolution and the dimmed look are tested. Clicking a link has not been tried.
- [x] **FR-PREV-09** Local images and `![[img|300]]` render, missing ones show a named box, and remote images don't load.
  - Checked: local, missing and remote images are each seen to draw
- [x] **FR-PREV-10** Raw HTML shows as text, and `<br>` breaks the line.
  - Checked: source tests

Filters, list and palette
- [x] **FR-FILT-01** Search, priority chips, assignee, label and due filters work alone and combined, and reset clears them.
  - Checked: board and app tests
- [x] **FR-FILT-02** Save view, apply view and delete view work per project and persist.
  - Checked: app test
- [x] **FR-FILT-03** Board and List switch, list rows open cards, and the choice persists.
  - Checked: app test
- [x] **FR-PAL-01** ⌘K searches cards, Enter opens one, "Create card ..." creates, "Move open card to ..." moves, and Esc closes.
  - Checked: app tests

Live updates
- [x] **FR-LIVE-02** mdk's own saves don't trigger a reload or a clash banner.
  - Checked: store tests on three OSes
- [x] **FR-LIVE-03** The Auto-refresh switch pauses and resumes updates and persists, and "checked" shows the time.
  - Checked: app test with the real watcher: paused updates do not arrive, and switching back on catches up
- [x] **FR-LIVE-04** A new subfolder created by an agent is picked up and watched.
  - Checked: store test

App shell
- [ ] **FR-APP-01** The header matches Electron's, with the "mdk" name.
  - Open: Seen in the real window, in both themes. Not compared side by side with Electron.
- [ ] **FR-APP-02** Dark (default), light and system themes, matching Electron's colours, and the choice persists.
  - Open: Colours converted from Electron's CSS, and the choice persists (tested). The match is not checked by eye.
- [x] **FR-APP-03** Every pref in Stage 5's Prefs table survives a restart.
  - Checked: prefs and app tests
- [ ] **FR-APP-04** Window size, position and maximised state are restored.
  - Open: Built. Not tested: a headless window has no real position.
- [x] **FR-APP-05** `N`, ⌘K, Esc, ⌘Z, ⌘⇧Z and ⌘, work (Ctrl on Windows and Linux).
  - Checked: app tests, with Cmd on macOS and Ctrl on Windows and Linux in CI
- [ ] **FR-APP-06** Native macOS menu, and in-window menu on Windows and Linux, with the main commands.
  - Open: Built. A headless window has no menus, so neither kind has been seen.
- [ ] **FR-APP-08** A second launch focuses the existing window.
  - Open: Tested on macOS and Linux with a local socket. The Windows named pipe compiles and its test runs in CI, but a second launch has not been tried by hand on any OS.

First run
- [x] **FR-FIRST-01** With no projects, the welcome state shows Connect folder and Create sample project.
  - Checked: app test
- [x] **FR-FIRST-02** Create sample project writes `mdk-sample/` with cards in all five columns covering every preview feature, and connects it.
  - Checked: app test
- [x] **FR-FIRST-03** No matching cards shows "No cards match" with Create first card.
  - Checked: app test

Agent contract and release
- [x] **FR-AGENT-01** `AGENTS.md` and `skills/mdk/SKILL.md` ship in the repo, and the Electron format rules hold unchanged.
  - Checked: both files are in the repo
- [ ] **FR-PKG-01** A `v*` tag builds a dmg, an NSIS exe and an AppImage on a GitHub release, and each installs and runs.
  - Open: No tag has been pushed. On this Mac the `.app` and the `.dmg` both build (11.0 MB). The Windows and Linux packages and the release workflow have never run.
- [ ] **FR-PKG-02** The README covers install, unsigned-app steps, the file format, agents and the before and after numbers.
  - Open: Everything is in the README. The speed numbers are for an empty board, not the 169-card one.

Non-functional
- [ ] **NFR-01** mdk beats Electron on download size, cold start, idle memory and edit-to-screen time (numbers in the README). Filter typing updates within 100 ms on 1,000 cards.
  - Open: Download size, app size and idle memory beat Electron (11 MB against 196 MB, 67 MB against 250 MB idle). Launch to first window ties. Edit-to-screen time is not compared. Memory and launch were measured on an empty board.
- [ ] **NFR-02** No network crates in `Cargo.lock` (cargo-deny ban passes). No requests made (checked with Little Snitch or an equivalent).
  - Open: `cargo-deny` bans pass in CI. No traffic check has been run.
- [ ] **NFR-03** Every feature works with networking turned off.
  - Open: Not checked by hand. There is no network code to fail.
- [ ] **NFR-04** A generated 20-project, 5,000-card setup loads and stays usable; a 1 MB card and a 20 MB image open.
  - Open: 1,000 cards are tested. 20 projects, 5,000 cards, a 1 MB card and a 20 MB image are not.
- [ ] **NFR-05** The release checklist passes on macOS 13+, Windows 11 and Ubuntu 22.04.
  - Open: Not started. It needs the three machines.
- [ ] **NFR-06** All UI is keyboard reachable with visible focus, the contrast script passes both themes (4.5:1 text, 3:1 UI), and reduce motion is respected.
  - Open: The contrast test passes for both themes. Keyboard reach is partly tested. Reduce motion is read from the OS by gpui-kit at start and again when the window is activated, but nobody has switched it on and looked.
- [x] **NFR-07** No test or manual run ever leaves a truncated or corrupted card; files mdk can't parse are never rewritten by board actions.
  - Checked: store, board and editor tests
- [ ] **NFR-08** Electron and mdk open the same folder at once, editing in both, with no corruption.
  - Open: Not run side by side. The format is byte-compatible by test.
- [x] **NFR-09** `mdk-format`, `mdk-store`, `mdk-markdown` and `mdk-vim` build and test with no dependency on the `mdk` app crate.
  - Checked: the four crates build and test alone in CI
- [x] **NFR-10** `cargo-deny` licence check passes, and `THIRD_PARTY_LICENSES.html` is bundled.
  - Checked: `cargo-deny` passes in CI, and the notices file is generated and bundled in the local `.app`

## M8 notes

Built: the sample project (seven cards over all five columns and three work folders, embedded in the binary, written by "Create sample project" with a copy of `AGENTS.md`), `AGENTS.md` and `skills/mdk/SKILL.md`, a log file, `Packager.toml`, the release workflow, the licence notices, `scripts/bench.sh`, a contrast test and a scale test.

Checked:

- The sample project fills all five columns, parses with no errors, and creating it twice overwrites nothing (app test).
- `cargo packager --release -c Packager.toml --formats app` builds `dist/mdk.app` on this Mac: 28 MB with the licence notices, bundle id `com.abdulkadersafi.mdk`, minimum macOS 13.0. The release binary starts, scans both task folders in the Electron repo (35 cards) and writes `mdk.log`.
- `cargo about` generates the notices: 12 licence kinds, none of them GPL.
- The contrast test reads the shipped theme file. It failed at first: the card's file path and the sidebar's "checked" time were at 70% opacity (as in Electron) and fell below 4.5:1 in both themes, and two light-theme badge colours were short. All four are fixed.
- On 1,000 cards in a release build, a scan takes 31 ms and a filter keystroke 0.3 ms.

Not checked: the `.dmg` (its Finder styling script timed out because the screen was locked), the NSIS installer, the AppImage, the release workflow (it only runs on a tag), and `scripts/bench.sh` end to end.

Decisions and deviations:

- The agent skill is not a straight copy. Electron's table of what the preview renders names features mdk does not have (line highlights, diff markers, HTML tags), so the table is rewritten to what mdk renders.
- `AGENTS.md` gains two lines: agents may add their own keys, and frontmatter must stay valid YAML.
- The contrast check is a Rust test, not a script, so it runs in CI.
- `AGENTS.md` in the sample folder is a card, because every `.md` file is (FR-FILE-01). It lands in Backlog.
- The log uses `env_logger` writing to the file. Rotation happens at start, not while running.
- `THIRD_PARTY_LICENSES.html` is generated and not committed. Packaging needs it to exist.
- `![[image.png]]` also looks at the project root, as Electron does.

## M7 notes

Built: the filter bar (search across title, body, id, assignee and labels; four priority chips; assignee; labels and due dropdowns; reset; a filtered or all badge), saved views per project with chips and a delete cross, the Board and List switch, the list view with sortable headers, the command palette on Cmd or Ctrl+K (up to 8 cards, "Create card", "Move open card to", and the app commands), the settings dialog on Cmd or Ctrl+comma, the native menu on macOS and gpui-kit's menu bar in the window on Windows and Linux, and connecting a folder by dropping it on the window.

Checked:

- `crates/mdk/tests/board_queries.rs`, 6 tests: each filter alone and combined, with Electron's rules; the label list follows the selection and not the filters; a saved view survives the config file; the palette's items, their order and the cap of 8 cards.
- `crates/mdk/tests/app.rs`, 6 more tests in a headless window: typing in the search box filters every column; chips and dropdowns combine; a filter that hides everything shows the empty state; saved views belong to their project and persist; the list view shows, sorts and opens with the keyboard; the palette opens a card, moves the open card and creates a card with quick-add; Up and Down stop at the ends; the settings shortcut; an app command run from the palette.

Not checked: the native macOS menu and the in-window menu bar (neither exists in a headless window), dropping a folder from the OS, the two dropdowns' placement on screen, and "every shortcut works on all three OSes" by hand.

Decisions:

- "This week" is due today or in the next six days, by date. Electron compares milliseconds from now.
- The palette lists title matches first, then by board order, and leaves archived cards out. "Create card" from the palette understands quick-add words and opens the new card.
- A list header click sorts ascending, a second click descending, a third goes back to board order.
- The labels and due dropdowns are hand-built, like the card's move menu.
- In list view, Up and Down move the focused row and Enter opens it.

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
