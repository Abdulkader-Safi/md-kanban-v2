# Build spec: mdk (native MD Kanban)

> **This is the hand-off document for Claude Code.** It is self-contained: build from this file alone. It pulls together the product, users, requirements and tech decisions from stages 1 to 5. Where it says "see Stage N", that is optional background.

Status: signed off 2026-10-11. Sources: 01 to 05 as of 2026-10-11.

## 1. What we're building

**mdk** is a native desktop rebuild of MD Kanban (github.com/Abdulkader-Safi/md-kanban, Electron plus Vue, commit `edebd29`) in Rust with GPUI and gpui-kit. It is a kanban board for watching what AI coding agents are doing.

- Every connected folder is a project, and every first-level subfolder is a work group. Every `.md` file is a card, with YAML frontmatter holding status, priority and the other fields.
- Agents move cards by editing files. mdk watches the folders and updates live.
- Users get the same UI and features as the Electron app: drag and drop, filters, saved views, list view, ⌘K palette, quick-add syntax, a markdown editor with a toolbar and Vim keys, and a rich preview (code highlighting, maths, Mermaid, callouts, wikilinks).
- It runs on macOS, Windows and Linux.

The goals: beat Electron on size, startup, memory and update latency; become a reusable GPUI foundation for later apps; and be a portfolio piece. MIT, free, no network, no accounts.

The Electron app stays alive in its own repo. **Both apps must work on the same folders at the same time.** The file format is a contract with agents and with the Electron app, and must not change.

## 2. Stack (decided, see Stage 4)

| Layer | Choice |
|-------|--------|
| Language | Rust (stable, edition 2024), Cargo workspace |
| UI framework | `gpui-kit = "=0.7.1"` (Apache-2.0). GPUI comes from it via `gpui-pre =0.3.8`. Do not add `gpui` directly. Do not `cargo update` gpui-kit casually |
| Components | gpui-kit: button, input, select, date-picker, dialog, alert-dialog, sheet, popover, tooltip, notification, command, sidebar, tabs, toggle-group, checkbox, switch, badge, tag, scrollable, virtual-list, data-table, kbd, menu / app menu bar, title-bar |
| Drag and drop | GPUI built-in `on_drag`, `drag_over`, `on_drop`, `on_drag_move`, with a card-face drag preview and drop-slot elements |
| Markdown editor | gpui-kit `Editor` and `EditorState` with the markdown tree-sitter grammar |
| Vim | own crate `mdk-vim`, keys captured with GPUI `capture_key_down` before the editor |
| Markdown preview | gpui-kit `TextView` (markdown-rs `markdown` 1.0) plus `MarkdownPlugin`s and `on_link_click` |
| Maths | `ratex-gpui` 0.4.1 (MIT) |
| Mermaid | `mermaid-rs-renderer` 0.3.1 (MIT), SVG drawn via GPUI `svg()` or rasterised with `resvg` at gpui-kit's version. Fallback: pin `zed-industries/mermaid-rs-renderer` by commit |
| YAML | `serde-saphyr` 1.3.0 for reading. Own writer for output |
| File watching | `notify` 8.2.0 plus `notify-debouncer-full` 0.7.0, 150 ms debounce |
| Config | `directories` 6.0.0 plus `serde_json`, one `config.json` |
| Theme | custom gpui-kit theme JSON (dark, light, system) copied from Electron's CSS variables, square chips, contrast checked by script |
| Undo | app-level command stack in `Board` (inverse ops, original bytes kept for delete and archive) |
| Icons | about 25 Tabler SVGs (MIT) copied from the Electron app's icon set |
| Fonts | Geist Sans (OFL 1.1) bundled. System monospace (Menlo, Consolas, DejaVu Sans Mono) |
| Logging | `log` plus `env_logger` to a rotated `mdk.log` |
| Packaging | `cargo-packager` 0.11.8: macOS aarch64 `.dmg`, Windows x64 NSIS `.exe` (per-user), Linux x64 AppImage. Unsigned |
| CI | GitHub Actions on `macos-14`, `windows-latest` and `ubuntu-22.04`: fmt, clippy `-D warnings`, test; release on `v*` tags; `cargo-deny` for licences (no GPL or AGPL) and network crate bans |
| Third-party notices | `cargo-about` generates `THIRD_PARTY_LICENSES.html`, bundled |
| Testing | `cargo test` (pure crates, golden fixtures from Electron's parser, watcher tests on 3 OS), GPUI `TestAppContext` view tests, `scripts/bench.sh` for NFR-01 |
| Licence | MIT |
| Platforms | macOS 13+ (Apple Silicon), Windows 10 and 11 x64, Linux x64 (Ubuntu 22.04+, Fedora 40+, X11 and Wayland) |

## 3. Architecture and principles

```
mdk (app crate, GPUI)
  Views (gpui-kit): Root, Header, Sidebar, FilterBar, BoardView | ListView,
                    CardEditorSheet, NewCardDialog, CommandPalette, Toasts
        │ read + dispatch actions
  Board (Entity): projects, cards, selection, filters, UndoStack, pending writes
        │ write (only here)                 ▲ StoreEvent via channel
  mdk-store: scan, read, atomic write, path guard, notify watcher (bg thread)
        │ uses
  mdk-format: parse/write cards, quick-add, due labels, comments, subtasks, slugs
  mdk-markdown: toolbar commands + TextView plugins (callout, wikilink, highlight,
                math, mermaid, code block, image, raw html), dollar guard
  mdk-vim: pure Vim state machine over a VimBuffer trait
        │
  project folders on disk  ◄── AI agents and the Electron app edit the same files
```

**Non-negotiable principles:**
1. **The files are the truth.** mdk keeps no database and writes nothing inside project folders except card files: no lock files, caches or dotfiles. All app state lives in the OS config folder.
2. **Single writer.** Only `Board` methods write card files, and every write does: re-read the file, apply only the changed fields, write atomically (temp file, `fsync`, rename), record the content hash, push undo. Views and the watcher never write.
3. **Never damage an agent's data.** Unknown frontmatter keys are copied verbatim. Line endings are kept. A file with broken YAML is never rewritten by board actions. An agent's edit to a card is never overwritten by stale state.
4. **Byte-compatible with Electron.** Writing uses exactly Electron's key order, quoting and layout (section 5). The Electron app must read mdk's files identically, and the other way round.
5. **No network.** No HTTP client crates. Remote images are not loaded. Only `http`, `https` and `mailto` links open, through the OS.
6. **Pure core, thin UI.** `mdk-format`, `mdk-store`, `mdk-markdown` (commands and parsing) and `mdk-vim` must not depend on the board UI, and are tested without a window.
7. **Pinned GPUI.** gpui-kit stays at `=0.7.1` unless an upgrade is done on purpose, in its own commit, with CI green on all three OSes.
8. **MIT clean.** Never copy code from Zed (editor, vim, markdown_preview are GPL or AGPL) or from Zorite (GPLv3). Port from the Electron repo (MIT, same author) freely.

## 4. Repository structure

```
mdk/                         (new repo: github.com/Abdulkader-Safi/mdk)
├── Cargo.toml               workspace, exact pins, [profile.release] lto = "thin", strip = true
├── deny.toml                cargo-deny: licences allowlist, ban reqwest/ureq/hyper/GPL
├── Packager.toml            cargo-packager config for dmg / nsis / appimage
├── AGENTS.md                agent contract, copied from Electron repo (name changed to mdk)
├── README.md                install per OS, unsigned-app steps, file format, agents, before/after numbers
├── LICENSE                  MIT
├── PROGRESS.md              milestone log (see section 13)
├── skills/mdk/SKILL.md      Claude skill, copied from Electron's skills/md-kanban
├── assets/
│   ├── icons/               Tabler SVGs used by the UI
│   ├── fonts/               Geist Sans (OFL licence file included)
│   ├── theme/mdk.json       gpui-kit theme, dark + light, from Electron's style.css
│   ├── app-icon.svg / .icns / .ico / .png
│   └── sample/              sample project cards embedded with include_str!
├── crates/
│   ├── mdk-format/          parse/write cards, quick-add, templates, due format, comments, subtasks, slugs
│   │   └── tests/fixtures/  every card from Electron's tasks/ + examples/ and expected JSON
│   ├── mdk-store/           Project, scan, read, atomic write, path guard, watcher, StoreEvent
│   ├── mdk-markdown/        commands.rs (toolbar), plugins/*.rs, dollar_guard.rs
│   ├── mdk-vim/             vim.rs (state machine), buffer.rs (VimBuffer trait), tests
│   └── mdk/                 the app: main.rs, board.rs, undo.rs, prefs.rs, actions.rs, theme.rs,
│                            views/{header,sidebar,filter_bar,board_view,column,card,list_view,
│                            editor_sheet,new_card,palette,settings,toasts,welcome}.rs
├── spikes/                  M0 throwaway spikes (deleted or archived after M0)
├── scripts/bench.sh         NFR-01 measurement (mdk vs Electron)
└── .github/workflows/       ci.yml (3 OS test), release.yml (tagged builds)
```

## 5. Data model

All times in files are ISO strings, kept as read. A card's key inside mdk is **project id plus relative path**, never the frontmatter `id` (ids can repeat).

- **Project** : `id` (8-char base36), `name` (folder name), `root` (absolute, canonical path), `order`, and runtime `status` (`Ok | Missing | Unreadable(msg)`).
- **Card** : `key`, `id`, `title`, `body`, `status`, `priority`, `assignee`, `due_date: Option<String>`, `created`, `modified`, `labels: Vec<String>`, `order: f64`, `workspace`, `file_name`, `extra: Vec<RawKey{name, source_lines}>`, `line_ending: Lf|CrLf`, `parse_error: Option<String>`, `raw: Arc<str>`, `content_hash: u64`. Derived: `excerpt`, `subtasks`, `search_text`.
- **Status** : `backlog`, `todo`, `in-progress`, `review`, `done`. Columns are Backlog `#6b7280`, To Do `#3b82f6`, In Progress `#f59e0b`, Review `#8b5cf6`, Done `#22c55e`.
- **Priority** : `critical`, `high`, `medium`, `low`.
- **Filters** : `query`, `priorities`, `assignee`, `label`, `due` (`all|overdue|today|week|none`). **SavedView** : `name` plus `filters`, stored per project id (or `"all"`).
- **Prefs** (`config.json`, `version: 1`): projects, selected project and workspace, auto_refresh, vim_keys, theme (`dark|light|system`), view (`board|list`), editor_tab, editor_expanded, saved_views, and window geometry.
- **UndoEntry** : `label` plus `ops` (`Write{key, before, after}`, `Rename{from, to}`, `Delete{key, bytes}`), in memory for the session.
- **StoreEvent** : `Changed`, `Removed`, `Renamed`, `Rescan`, `RootMissing`, `Error`.

**Reading a card (must match Electron's `parseTaskFile`):**
- Strip a leading BOM.
- Frontmatter exists only if line 1 trimmed is `---` and a later line trimmed is `---`. Otherwise the whole file is the body.
- Body = text after the closing `---`, trimmed.
- Missing `id` = file name without `.md`.
- `status` not one of the five = `backlog`. `priority` not one of the four = `medium`.
- `assignee` = text or `""`. `dueDate` = text or `null`.
- `created` and `modified` default to now if missing.
- `labels` = a string array, or `[]`.
- `order` = the number, or 0.
- Title = text of the first line matching `^#\s+(.+)$` in the body, trimmed, or the id.

**Writing a card (must match Electron's `stringifyTaskFile` byte for byte, plus extras):**
```
---
id: "<id>"
status: "<status>"
priority: "<priority>"
assignee: "<assignee or empty>"
dueDate: "<YYYY-MM-DD>"        (or:  dueDate: null)
created: "<created as read>"
modified: "<now, YYYY-MM-DDTHH:MM:SS.sssZ>"
labels: ["a", "b"]             (or:  labels: [])
order: <number, JS formatting: 3, 2.5, 1728650000000>
<unknown keys: original source lines, verbatim, original order>
---
<body; if it has no "# " heading line, prepend "# <title>\n\n">
```
Strings are JSON-quoted (escape `"`, `\` and control characters, keep non-ASCII as is). The body ends with exactly one newline. CRLF files are written with CRLF throughout.

**New card file:** `slugify(title)` (lowercase, ASCII letters and digits, runs of anything else become `-`, trimmed, same as Electron's `slugify`), then `-YYYY-MM-DD.md`. The id is the same without `.md`. New cards get `order = current time in ms` (Electron behaviour). Name clashes get `-2`, `-3` and so on.

## 6. Command surface

On the `Board` entity (all writes go through these):
- **Projects:** `connect_project(path)`, `remove_project(id)`, `relocate_project(id, path)`, `rescan(id)`, `rescan_all()`, `select_project(id|All)`, `select_workspace(name|All)`, `create_sample_project(parent_dir)`.
- **Cards:** `create_card(NewCard, workspace)`, `update_card(key, CardPatch)`, `move_card(key, status, index: Option<usize>)`, `delete_card(key)`, `archive_done(scope)`, `toggle_subtask(key, i)`, `undo()`, `redo()`, `resolve_conflict(key, KeepMine|LoadTheirs)`.
- **Queries:** `visible_cards()`, `column(status)`, `workspaces()`, `labels_in_view()`, `open_counts()`, `palette_items(query, open_card)`, `resolve_link(from, target)`, `read_image(key, rel_path)`.

`move_card` order maths (port of Electron's `moveTaskAt`):
- Index `None`, or past the end: `order = max(other orders in the column) + 1`, or 0 if the column is empty.
- Index 0: first card's order minus 1.
- Otherwise: the midpoint of the neighbours.
- If the result isn't finite or equals a neighbour, renumber the column `0, 10, 20 ...` and use `idx * 10 - 5`.

`mdk-format`: `parse`, `write`, `apply_patch`, `parse_quick_add(input, today)`, `template(bug|feature|chore, title)`, `format_due(due, today) -> (label, tone)`, `strip_comments`, `subtasks`, `toggle_subtask`, `slugify`, `new_file_name`.

`mdk-markdown`:
- Commands: `wrap`, `link(image)`, `heading(n)`, `toggle_list(bullet|ordered|task|quote)`, `code_block`, `table`, `callout`, `math_block`, `mermaid`, `rule` (ports of Electron's `md-commands.ts`).
- Plugins: `CalloutPlugin`, `WikiLinkPlugin`, `HighlightPlugin`, `MathPlugin`, `MermaidPlugin`, `CodeBlockPlugin`, `ImagePlugin`, `RawHtmlPlugin`.
- Also `dollar_guard`.

`mdk-vim`: `Vim::handle(keystroke, &mut dyn VimBuffer) -> Handled`. `VimBuffer` offers text, cursor, selection, replace range, undo, redo and search.

GPUI actions: `NewCard (n)`, `TogglePalette (secondary-k)`, `Undo (secondary-z)`, `Redo (secondary-shift-z)`, `CloseTop (escape)`, `OpenSettings (secondary-,)`, `ConnectFolder`, `RescanAll`, `ToggleTheme`, `ToggleVim`, `ShowBoard`, `ShowList`, `ArchiveDone`, and on a focused card `PickUp/Drop (space)`, `MoveLeft/Right/Up/Down (arrows)`, `CancelMove (escape)`.

Errors are one `MdkError` enum (`Io`, `OutsideRoot`, `Parse`, `Conflict`, `NotFound`, `BrokenFrontmatter`), shown as plain-English toasts.

## 7. Screens

Match the Electron UI. The reference screenshot is `docs/spec/references/electron-board-screenshot.png`. The reference source is the Electron repo's `src/App.vue` and `src/components/*.vue`.

- **Main window:**
  - **Header:** app mark, "mdk", "local markdown board · N cards · N projects", the active project chip, "Search ⌘K", theme toggle, and a teal primary "New card N".
  - **Sidebar (240 px):**
    - Connect folder (teal).
    - PROJECTS: All projects, then each project with its open count. The selected project shows rescan and remove, or "Locate folder" when missing.
    - WORK FOLDERS in monospace: all, then the folders.
    - Footer: Rescan all, Auto-refresh switch, Vim keys switch, "checked HH:MM:SS".
    - Below 1024 px the sidebar collapses behind a header button.
  - **FilterBar:**
    - Search input "Search title, body, id, assignee, label...".
    - critical, high, medium and low toggle chips.
    - Assignee input, "all labels" select, "any due" select, "reset", and the "filtered" or "all" badge.
    - "save view", then saved-view tags with ×.
    - Board/List segmented control on the right.
  - **BoardView:** five 288 px columns, scrolling horizontally. Each column has a header (dot, name, count, "archive" on Done, "+"), cards with drop slots, and a dashed "+ Add card".
  - **CardView:**
    - Title, priority badge, and the "⋯" move menu.
    - Two-line excerpt.
    - Chips: @assignee, due (red when overdue, amber today), work folder in monospace, ☑ done/total.
    - Up to 3 #labels plus "+N more", then the file path in small monospace.
    - States: hover border, focus ring, dragging (dimmed), picked up with the "Moving: arrows shift, Space drops, Esc cancels" hint, broken-YAML warning.
  - **ListView:** a data-table with Title, Status, Priority, Due and Assignee. Rows open the card. Headers sort.
  - **States:**
    - First-scan loading: "Loading folders...".
    - Welcome, when there are no projects: Connect folder and Create sample project.
    - "No cards match", with Create first card.
    - Project error rows, and toasts.
- **Card editor sheet** (right side, 672 px, Expand makes it two-thirds of the window):
  - Path line and title input, plus X.
  - Grid: status, priority, assignee and due (date picker), then labels.
  - A clash banner or deleted banner when needed.
  - Markdown and Preview tabs, plus Expand.
  - **Markdown tab:** toolbar of 20 buttons (H1 H2 H3 | B I S == code | link image | bullets numbers checklist quote | code-block table callout math mermaid divider), the editor, and the Vim mode line.
  - **Preview tab:** the Subtasks box, then the rendered markdown.
  - Footer: Copy AI prompt, "Autosaves to the markdown file.", and Delete.
  - A collapsible raw file view.
- **New card dialog:**
  - Title, with placeholder "Card title, try: Fix login p1 #backend @safi due:friday".
  - "Will set: ..." hint line, and Template: bug, feature, chore.
  - Description textarea.
  - Grid: status, priority, assignee, due, labels, work subfolder.
  - Cancel and Create card, then "Tip: press N anywhere to create a card."
- **Command palette:** input "Search cards, or type to create...", up to 8 cards with a status hint, "Create card ...", "Move open card to ..." and app commands, plus the footer "↑↓ navigate · Enter run · Esc close".
- **Settings dialog (Should):** theme, auto-refresh, Vim, and Open config folder.
- **Toasts:** bottom centre. "Moved to X" with Undo for 5 s, "Copied", and errors.

## 8. Build plan (milestones)

### M0: de-risk (do this first)
Throwaway spikes in `spikes/`, plus baseline numbers. No product code until every exit criterion passes or its fallback is agreed with Safi.
- **Drag and drop spike:** five columns of 200 fake cards each, using gpui-kit plus GPUI `on_drag`, `drag_over`, `on_drop` and `on_drag_move`. Needs drop slots, column highlight, and auto-scroll at the column and board edges.
- **Vim spike:** an `EditorState` with keys captured by `capture_key_down`. Prove `h j k l`, `i`, `Esc`, `dd`, `u`, visual `v` + `d`, and that the editor's own Escape and Cmd shortcuts don't fire in normal mode. Prove the selection can be set from code (also needed for the toolbar).
- **Preview spike:** one TextView rendering one card with a callout (folded), a wikilink, `==highlight==`, inline and block maths through `ratex-gpui`, a Mermaid flowchart through `mermaid-rs-renderer`, a highlighted code block with a header, and a local image.
- **Editor headings check:** can headings in the editor render larger than body text? Record the answer.
- **File spike:** a `notify` debouncer on a temp folder, plus atomic writes, run as a test on all three CI OSes: edit, rename-save (like vim or git), a 50-file burst, a write over a file another process has open on Windows.
- **Electron baseline:** with Electron at `edebd29` and Safi's 169-card board on an M1 with 16 GB, measure dmg size, cold start to the rendered board, idle RSS after 10 minutes, and edit-to-screen latency. Write the numbers into `PROGRESS.md`.
- **Exit criteria:**
  - Dragging is smooth (no frame over 33 ms in a 10 s drag) on macOS, and works on Windows and Linux.
  - The Vim spike passes on macOS.
  - All preview elements render.
  - The watcher test passes on all three CI runners.
  - Baseline numbers are recorded.
- **Fallbacks if a spike fails:**
  - Drag and drop: non-virtualised columns, with a cap and "show more".
  - Vim: the keys go through a custom key context instead of `capture_key_down`.
  - Maths: `ratex-svg` plus `resvg`.
  - Mermaid: Zed's fork pinned by commit, or Mermaid shown as source.
  - Headings: bold and coloured only, in which case FR-MDE-01 is amended with Safi's OK.

  **Stop and tell Safi** before taking any fallback.

### M1: core crates (no UI)
- **Builds:** FR-FILE-01, FR-FILE-02, FR-FILE-03, FR-FILE-04, FR-FILE-05, FR-FILE-06, FR-FILE-07, FR-FILE-08, FR-FILE-09, FR-AGENT-02, FR-AGENT-03 (store side), FR-LIVE-01, FR-LIVE-02, FR-LIVE-04 (store side), FR-NEW-02 and FR-NEW-03 (parsing and templates), FR-CARD-01 (due label maths).
- **Tasks:**
  - Workspace, CI (3 OS), `deny.toml`.
  - `mdk-format`: parse, write, apply_patch, quick-add, templates, due format, comments, subtasks, slugs.
  - Golden fixtures from Electron's `tasks/` and `examples/` cards. Expected JSON is generated once by running Electron's `parseTaskFile` under bun, with the generation script committed.
  - `mdk-store`: scan rules, atomic write, path guard (including symlinks), watcher with debounce, burst collapse, own-write hash suppression, re-read-merge-write.
- **Exit criteria:** `cargo test --workspace` green on all three OSes. The round-trip of every fixture is byte-identical except `modified`. The unknown-key test passes. The watcher latency test is under 500 ms.

### M2: app shell and read-only board
- **Builds:** FR-APP-01, FR-APP-02, FR-APP-03, FR-APP-04, FR-APP-08, FR-PROJ-01, FR-PROJ-02, FR-PROJ-03, FR-PROJ-04, FR-PROJ-05, FR-PROJ-06, FR-PROJ-07, FR-PROJ-08, FR-PROJ-09, FR-BOARD-01, FR-BOARD-02, FR-BOARD-03, FR-BOARD-04, FR-BOARD-06, FR-CARD-01, FR-CARD-02 (opens a placeholder sheet), FR-CARD-04, FR-FIRST-01, FR-FIRST-03, FR-LIVE-03.
- **Tasks:** window and single-instance, theme JSON from Electron's `style.css` (dark default, light, system), Tabler icons, Geist font, header, sidebar, board, columns, card face, prefs and config.json, window state, the welcome and empty states, and live updates wired from the store.
- **Exit criteria:**
  - Connecting Safi's real `MD-Kanban` and `clinicos` task folders shows the same cards in the same columns and order as Electron.
  - A side-by-side screenshot comparison with `electron-board-screenshot.png` is accepted by Safi.
  - Editing a card file in another editor moves it on the board.

### M3: moving and creating
- **Builds:** FR-MOVE-01, FR-MOVE-02, FR-MOVE-03, FR-MOVE-04, FR-MOVE-05, FR-CARD-03, FR-NEW-01, FR-NEW-02, FR-NEW-03, FR-NEW-04, FR-NEW-05, FR-BOARD-05, FR-AGENT-03.
- **Tasks:** drag and drop from the M0 spike, the move menu, keyboard moving, the undo stack (move, create, delete, archive) with ⌘Z and ⌘⇧Z, the 5-second undo toast, the New card dialog with quick-add hint and templates, and archive with its confirm dialog.
- **Exit criteria:**
  - Every move writes only `status`, `order` and `modified`.
  - An agent body edit followed by a user drag keeps both.
  - Undo of delete restores the file byte for byte.
  - Electron opens mdk-written files correctly.

### M4: card editor and markdown editor
- **Builds:** FR-EDIT-01, FR-EDIT-02, FR-EDIT-03, FR-EDIT-04, FR-EDIT-05, FR-EDIT-06, FR-EDIT-07, FR-EDIT-08, FR-EDIT-09, FR-EDIT-10, FR-MDE-01, FR-MDE-02, FR-MDE-03, FR-MDE-04.
- **Tasks:** the sheet with Expand, field grid with date picker, autosave with flush, title-heading sync, the toolbar on `mdk-markdown::commands` (ported `md-commands.check.ts` cases as tests), editor shortcuts, find, list continuation, Copy AI prompt (Electron's exact text), delete with confirm, raw file view, the clash and deleted banners, and editing broken frontmatter in the raw view.
- **Exit criteria:**
  - Typing and closing within 100 ms saves.
  - The clash banner appears when an external write races unsaved typing, and silent reload happens when the editor is clean.
  - All toolbar tests pass.

### M5: Vim mode
- **Builds:** FR-VIM-01, FR-VIM-02, FR-VIM-03.
- **Tasks:** `mdk-vim` state machine, the `VimBuffer` adapter for `EditorState`, the mode line, the sidebar switch, and Esc never closing the sheet in Vim mode.
- **Exit criteria:** the table-driven test for every FR-VIM-02 command passes. Manual check in the app.

### M6: preview
- **Builds:** FR-PREV-01, FR-PREV-02, FR-PREV-03, FR-PREV-04, FR-PREV-05, FR-PREV-06, FR-PREV-07, FR-PREV-08, FR-PREV-09, FR-PREV-10 (and FR-PREV-11, Should).
- **Tasks:** the TextView with all mdk plugins from the M0 spike, made production quality: the subtasks box, link handling (external links via the OS, card links opening the card, links outside the project as plain text), image loading through the store, the dollar guard, comment stripping, caching of Mermaid and maths renders, and the theme-aware colours. Port the `render.check.ts` cases.
- **Exit criteria:** the sample project's "preview showcase" card renders every element in both themes. All ported render tests pass.

### M7: filters, views, list and palette
- **Builds:** FR-FILT-01, FR-FILT-02, FR-FILT-03, FR-PAL-01, FR-APP-05, FR-APP-06 (and FR-FILT-04, FR-PAL-02, FR-APP-07, FR-PROJ-10, all Should).
- **Tasks:** the filter bar and logic (Electron's rules), saved views per project, the list view, the ⌘K palette, the full shortcut set, the native macOS menu and in-window menu on Windows and Linux, and the settings dialog.
- **Exit criteria:** filter combinations match Electron on the same data. Every shortcut works on all three OSes.

### M8: first run, agent contract, packaging, performance
- **Builds:** FR-FIRST-02, FR-AGENT-01, FR-PKG-01, FR-PKG-02.
- **Tasks:**
  - The sample project (embedded, written to a chosen folder, at least 6 cards covering all features).
  - `AGENTS.md` and `skills/mdk/SKILL.md`.
  - `Packager.toml`, `release.yml`, `cargo-about` notices.
  - The README with install steps per OS and the unsigned-app instructions.
  - `scripts/bench.sh`, then the NFR-01 numbers versus Electron in the README.
  - The contrast check script (NFR-06), and the manual release checklist on all three OSes.
- **Exit criteria:**
  - Tag `v0.1.0` produces a dmg, an NSIS exe and an AppImage on a GitHub release.
  - Each installs and runs on a clean machine or VM.
  - mdk beats Electron on all four NFR-01 numbers.
  - All acceptance boxes in section 9 are ticked.

## 9. Acceptance criteria

### POC (proves the core idea, end of M3)
- [ ] **FR-PROJ-01** Connect folder opens the native picker, and the chosen folder shows as a selected project with its cards.
- [ ] **FR-FILE-02** Every Electron fixture card parses to the same fields as Electron's parser.
- [ ] **FR-FILE-03** Every fixture round-trips byte for byte except `modified`, and Electron reads mdk-written files identically.
- [ ] **FR-FILE-04** A card with `sprint: 4` and `agent: "claude"` keeps both keys, verbatim, after a drag.
- [ ] **FR-LIVE-01** A script changing a card's `status` moves it on the board within 500 ms. A 50-file git checkout gives one refresh with no freeze.
- [ ] **FR-MOVE-01** Dragging a card between two cards in another column lands it there, and its file gets the new status and an in-between order.
- [ ] **FR-AGENT-03** An agent edits a closed card's body, the user drags it, and the file has both the new status and the agent's body.
- [ ] **FR-AGENT-02** After a session of creating, moving and deleting, `git status` shows only card `.md` changes.

### MVP (shippable v1, end of M8)
Projects and folders
- [ ] **FR-PROJ-02** Two connected folders survive a restart.
- [ ] **FR-PROJ-03** Project counts show cards that are not done (37 cards with 10 done shows 27).
- [ ] **FR-PROJ-04** Project selection filters the board and survives a restart.
- [ ] **FR-PROJ-05** Work folders list and filter correctly, including across All projects.
- [ ] **FR-PROJ-06** `archive/` cards are hidden unless the archive work folder is selected.
- [ ] **FR-PROJ-07** Remove project leaves files on disk unchanged.
- [ ] **FR-PROJ-08** Rescan all re-reads every project.
- [ ] **FR-PROJ-09** A renamed or moved folder shows "Folder not found" with Locate folder, and relocating restores it.

Card files
- [ ] **FR-FILE-01** Scanning follows the depth 4 and skip rules, and every `.md` is a card.
- [ ] **FR-FILE-05** Broken YAML shows a card with a warning. Dragging it is blocked with a toast, and the file is never rewritten by board actions.
- [ ] **FR-FILE-06** New files are `<slug>-YYYY-MM-DD.md` in the right folder, with `-2` on a clash.
- [ ] **FR-FILE-07** Writes are atomic (temp file plus rename). A test kills the write midway and the original is intact.
- [ ] **FR-FILE-08** UTF-8 with BOM is read. CRLF files stay CRLF after a save.
- [ ] **FR-FILE-09** Paths that leave the root (`..`, symlinks out) are refused. Non-image files can't be read as images.

Board and cards
- [ ] **FR-BOARD-01** Five columns with Electron's names and colours.
- [ ] **FR-BOARD-02** Column header shows dot, name, count and "+", plus "+ Add card" at the bottom.
- [ ] **FR-BOARD-03** Cards are sorted by `order`.
- [ ] **FR-BOARD-04** Horizontal board scroll, per-column vertical scroll, and no vertical overscroll of the board.
- [ ] **FR-BOARD-05** Archive moves all visible done cards into `archive/` after a confirm with the count.
- [ ] **FR-BOARD-06** A 1,000-card project scrolls and drags without frames over 33 ms.
- [ ] **FR-CARD-01** The card face matches the Electron screenshot for the same files.
- [ ] **FR-CARD-02** Clicking a card opens the editor sheet.
- [ ] **FR-CARD-03** The "⋯" menu moves the card to the end of the chosen column, with the current column disabled.
- [ ] **FR-CARD-04** Cards with broken frontmatter show a warning with the error on hover.

Moving
- [ ] **FR-MOVE-02** A move changes only `status`, `order` and `modified`, and colliding orders renumber the column.
- [ ] **FR-MOVE-03** The "Moved to X" toast's Undo within 5 s restores the status and order.
- [ ] **FR-MOVE-04** Keyboard moving with Space, arrows, Space and Esc works, Esc restores the original place, and the hint shows.
- [ ] **FR-MOVE-05** ⌘Z and ⌘⇧Z undo and redo a move, create, delete and archive. Undo of delete restores the exact bytes.

Creating
- [ ] **FR-NEW-01** New card opens from the header, `N`, column "+" and "+ Add card", with the right default status and work folder.
- [ ] **FR-NEW-02** `Fix login p1 #backend @safi due:friday` on 2026-10-11 gives "Fix login", critical, #backend, @safi, due 2026-10-16, and the hint line shows it.
- [ ] **FR-NEW-03** The bug, feature and chore templates fill priority, labels and body like Electron.
- [ ] **FR-NEW-04** The created card's file exists and the card shows at once.
- [ ] **FR-NEW-05** The dialog closes on Cancel, Esc and a backdrop press-and-release, but not when drag-selecting out of it.

Card editor
- [ ] **FR-EDIT-01** The sheet shows path, title, field grid with date picker, and labels. Expand works.
- [ ] **FR-EDIT-02** Typing then closing within 100 ms saves to the file.
- [ ] **FR-EDIT-03** Renaming the title updates the body's first `# ` heading.
- [ ] **FR-EDIT-04** Markdown and Preview tabs, with the last tab remembered.
- [ ] **FR-EDIT-05** Copy AI prompt puts Electron's prompt text on the clipboard and shows "Copied".
- [ ] **FR-EDIT-06** Delete confirms, removes the file and closes, and is undoable.
- [ ] **FR-EDIT-07** The raw file view shows the exact text that will be written.
- [ ] **FR-EDIT-08** With unsaved typing, an external change shows "Changed on disk" with Keep mine and Load theirs. When clean, it reloads within 500 ms.
- [ ] **FR-EDIT-09** An external delete shows "This file was deleted" with Recreate and Close.
- [ ] **FR-EDIT-10** The sheet closes on X, Esc (not in Vim mode) and backdrop press-and-release.

Markdown editor
- [ ] **FR-MDE-01** Markdown highlighting, wrapping, active line, selection matches and placeholder, in both themes.
- [ ] **FR-MDE-02** All 20 toolbar actions behave like Electron's, and the ported `md-commands.check.ts` cases pass.
- [ ] **FR-MDE-03** ⌘B, ⌘I, ⌘E, ⌘F, undo and redo, Tab and Shift-Tab, and list continuation on Enter.
- [ ] **FR-MDE-04** Selection, clipboard, word and line navigation, and IME input (tested with a Japanese or Arabic input method).

Vim
- [ ] **FR-VIM-01** The sidebar Vim switch toggles Vim in the editor and persists.
- [ ] **FR-VIM-02** Every command in the FR-VIM-02 list passes the table test, and the mode line shows the mode.
- [ ] **FR-VIM-03** Esc in Vim mode never closes the sheet.

Preview
- [ ] **FR-PREV-01** GFM renders: tables (scroll when wide), task lists, footnotes, strikethrough, blockquotes, rules and code.
- [ ] **FR-PREV-02** The Subtasks box ticks the right `- [ ]` line in the file, and the card face count updates.
- [ ] **FR-PREV-03** Code blocks are highlighted, with a language or title header and a working Copy.
- [ ] **FR-PREV-04** `$x^2$` and `$$\int$$` render as maths, "costs $5 a month, or $50" stays text, and a bad formula shows red source.
- [ ] **FR-PREV-05** A Mermaid flowchart renders in theme colours, and a broken one shows source and error.
- [ ] **FR-PREV-06** GitHub and Obsidian callouts render in five looks, and `-` and `+` folding works.
- [ ] **FR-PREV-07** `==highlight==` renders, and `%% comments %%` are hidden in the preview and on the card face but kept in the file.
- [ ] **FR-PREV-08** External links open in the browser, card links and wikilinks open the card, links outside the project are plain text, and unresolved wikilinks are dimmed.
- [ ] **FR-PREV-09** Local images and `![[img|300]]` render, missing ones show a named box, and remote images don't load.
- [ ] **FR-PREV-10** Raw HTML shows as text, and `<br>` breaks the line.

Filters, list and palette
- [ ] **FR-FILT-01** Search, priority chips, assignee, label and due filters work alone and combined, and reset clears them.
- [ ] **FR-FILT-02** Save view, apply view and delete view work per project and persist.
- [ ] **FR-FILT-03** Board and List switch, list rows open cards, and the choice persists.
- [ ] **FR-PAL-01** ⌘K searches cards, Enter opens one, "Create card ..." creates, "Move open card to ..." moves, and Esc closes.

Live updates
- [ ] **FR-LIVE-02** mdk's own saves don't trigger a reload or a clash banner.
- [ ] **FR-LIVE-03** The Auto-refresh switch pauses and resumes updates and persists, and "checked" shows the time.
- [ ] **FR-LIVE-04** A new subfolder created by an agent is picked up and watched.

App shell
- [ ] **FR-APP-01** The header matches Electron's, with the "mdk" name.
- [ ] **FR-APP-02** Dark (default), light and system themes, matching Electron's colours, and the choice persists.
- [ ] **FR-APP-03** Every pref in Stage 5's Prefs table survives a restart.
- [ ] **FR-APP-04** Window size, position and maximised state are restored.
- [ ] **FR-APP-05** `N`, ⌘K, Esc, ⌘Z, ⌘⇧Z and ⌘, work (Ctrl on Windows and Linux).
- [ ] **FR-APP-06** Native macOS menu, and in-window menu on Windows and Linux, with the main commands.
- [ ] **FR-APP-08** A second launch focuses the existing window.

First run
- [ ] **FR-FIRST-01** With no projects, the welcome state shows Connect folder and Create sample project.
- [ ] **FR-FIRST-02** Create sample project writes `mdk-sample/` with cards in all five columns covering every preview feature, and connects it.
- [ ] **FR-FIRST-03** No matching cards shows "No cards match" with Create first card.

Agent contract and release
- [ ] **FR-AGENT-01** `AGENTS.md` and `skills/mdk/SKILL.md` ship in the repo, and the Electron format rules hold unchanged.
- [ ] **FR-PKG-01** A `v*` tag builds a dmg, an NSIS exe and an AppImage on a GitHub release, and each installs and runs.
- [ ] **FR-PKG-02** The README covers install, unsigned-app steps, the file format, agents and the before and after numbers.

Non-functional
- [ ] **NFR-01** mdk beats Electron on download size, cold start, idle memory and edit-to-screen time (numbers in the README). Filter typing updates within 100 ms on 1,000 cards.
- [ ] **NFR-02** No network crates in `Cargo.lock` (cargo-deny ban passes). No requests made (checked with Little Snitch or an equivalent).
- [ ] **NFR-03** Every feature works with networking turned off.
- [ ] **NFR-04** A generated 20-project, 5,000-card setup loads and stays usable; a 1 MB card and a 20 MB image open.
- [ ] **NFR-05** The release checklist passes on macOS 13+, Windows 11 and Ubuntu 22.04.
- [ ] **NFR-06** All UI is keyboard reachable with visible focus, the contrast script passes both themes (4.5:1 text, 3:1 UI), and reduce motion is respected.
- [ ] **NFR-07** No test or manual run ever leaves a truncated or corrupted card; files mdk can't parse are never rewritten by board actions.
- [ ] **NFR-08** Electron and mdk open the same folder at once, editing in both, with no corruption.
- [ ] **NFR-09** `mdk-format`, `mdk-store`, `mdk-markdown` and `mdk-vim` build and test with no dependency on the `mdk` app crate.
- [ ] **NFR-10** `cargo-deny` licence check passes, and `THIRD_PARTY_LICENSES.html` is bundled.

## 10. Standard states
- **Undo and redo:** FR-MOVE-03, FR-MOVE-05, FR-MDE-03, FR-VIM-02.
- **Empty states:** FR-FIRST-01, FR-FIRST-03, and empty columns (FR-BOARD-02).
- **Loading:** first-scan "Loading folders..." only. Later refreshes are silent and keep the old cards.
- **Errors:** FR-PROJ-09, FR-FILE-05, FR-EDIT-09, FR-PREV-04, FR-PREV-05, plus write-failure toasts that keep the edit dirty.
- **Confirm before destructive actions:** FR-BOARD-05, FR-EDIT-06. Remove project never deletes files.
- **Autosave:** FR-EDIT-02.
- **Search and filter:** FR-FILT-01, FR-PAL-01, FR-MDE-03.
- **Shortcuts:** FR-APP-05, FR-MDE-03, FR-MOVE-04, FR-VIM-02.
- **Persistent settings:** FR-APP-03 (and FR-APP-07, Should).
- **First run:** FR-FIRST-01, FR-FIRST-02.
- **Offline:** always offline (NFR-02, NFR-03).
- **Delete and restore:** FR-EDIT-06 plus FR-MOVE-05. Archive is the soft delete.
- **Accessibility:** NFR-06 (keyboard and contrast).
- **Out of scope:** roles and permissions (single user), OS notifications, import and export (files are the format), audit log (git is the log), RTL and other languages, screen readers, mobile layout, rate limits (no network).

## 11. Out of scope for MVP
Don't build these, even if they look easy:
- A browser version and demo projects.
- Screen readers.
- RTL and Arabic UI.
- Vim registers, macros, marks, text objects, and ex commands beyond `:w` and `:q`.
- Raw HTML rendering.
- Remote images.
- Custom columns or statuses.
- Sync, accounts or a hosted board.
- Running or orchestrating agents.
- OS notifications and due-date reminders.
- Mobile and tablet.
- Signing, notarisation and auto-update.
- Intel Mac builds.
- Anything not in this spec.

## 12. Top risks and mitigations
1. **Card drag and drop across five scrolling columns in GPUI.** No reference kanban exists. *Mitigation:* the M0 spike on all three OSes. Fallback: non-virtualised columns with "show more".
2. **Vim on gpui-kit's Editor** (key capture order, setting selection from code). *Mitigation:* the M0 spike. Fallback: a custom key context. Pure `mdk-vim` logic tested without GPUI.
3. **Preview plugins** (wikilinks in text nodes, callout folding, ratex-gpui, mermaid-rs-renderer). *Mitigation:* the M0 spike. Fallbacks: `ratex-svg` plus `resvg`, Zed's Mermaid fork, or Mermaid as source with Safi's OK.
4. **Cross-platform watching and atomic writes** (Windows rename over open files, inotify limits, rename-saves). *Mitigation:* the M0 watcher tests on all three CI runners, debouncer pairing, own-write hashes, and a toast on watch-limit errors.
5. **GPUI and gpui-kit API churn.** *Mitigation:* exact pins, upgrades only in a dedicated commit with green CI on three OSes, and no direct `gpui` dependency.
6. **Byte compatibility with Electron.** *Mitigation:* golden fixtures from the real Electron parser and an M3 cross-check that Electron reads mdk-written files.

## 13. Instructions for Claude Code
- Build milestone by milestone, in order. Finish M0 before any feature work, and stop to report each M0 result to Safi.
- Keep a `PROGRESS.md` at the repo root: milestone status, decisions made, deviations from this spec and why, and the M0 baseline numbers.
- At the end of each milestone, run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`, then tick the acceptance boxes it covers (copy section 9 into `PROGRESS.md`). Don't move on with a failing box.
- Don't add features outside this spec. If something seems missing, note it in `PROGRESS.md` and ask.
- If a spec decision turns out to be wrong in practice (for example a gpui-kit API doesn't exist as described), stop and explain. Don't silently swap a stack choice or a crate.
- Port behaviour from the Electron repo (github.com/Abdulkader-Safi/md-kanban at `edebd29`, MIT, same author) by reading its TypeScript: `src/lib/markdown.ts`, `quick-add.ts`, `format.ts`, `comments.ts`, `md-commands.ts`, `render.ts`, `src/stores/board.ts`, `electron/store.ts`, and the Vue components for layout and copy text. Never copy code from Zed or Zorite (GPL).
- Never change the card file format. If you think it should change, ask.
- Commit after each milestone with a message naming the milestone (`M2: app shell and read-only board`).
- Keep secrets out of the repo (there are none: no API keys exist in this app). Config lives in the OS config folder, and env vars are `MDK_LOG`, `MDK_CONFIG_DIR` and `MDK_NO_WATCH`.

---

*Companion files in `docs/spec/`: 01-product, 02-users, 03-requirements, 04-tech-decisions, 05-tech-requirements, and `references/electron-board-screenshot.png`. This build spec is the entry point.*
