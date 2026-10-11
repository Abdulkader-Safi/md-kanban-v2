# Technical requirements

Status: signed off 2026-10-11

The engineering blueprint. Tech context: Rust workspace (`mdk-format`, `mdk-store`, `mdk-markdown`, `mdk-vim`, `mdk` app) on `gpui-kit =0.7.1` (GPUI via `gpui-pre =0.3.8`), `serde-saphyr` for YAML, `notify` 8.2 plus `notify-debouncer-full` 0.7 for watching, TextView plugins with `ratex-gpui` and `mermaid-rs-renderer` for the preview, cargo-packager for releases. MIT.

## Architecture

```
mdk (app crate, GPUI)
  ┌───────────────────────────────────────────────────────────────────┐
  │ Views (gpui-kit)                                                  │
  │  Root ─ Header ─ Sidebar ─ FilterBar ─ BoardView | ListView       │
  │        ─ CardEditorSheet ─ NewCardDialog ─ CommandPalette ─ Toasts│
  │            │ read + dispatch actions                              │
  │  ┌─────────▼──────────┐   ┌──────────────┐   ┌─────────────────┐  │
  │  │ Board (Entity)     │◄──│ UndoStack    │   │ Prefs (Entity)  │  │
  │  │ projects, cards,   │   └──────────────┘   │ config.json     │  │
  │  │ selection, filters │                      └─────────────────┘  │
  │  └───┬───────────▲────┘                                           │
  └──────┼───────────┼────────────────────────────────────────────────┘
         │ write     │ StoreEvent (channel -> cx.spawn)
  ┌──────▼───────────┴───────┐   ┌───────────────┐  ┌───────────────┐
  │ mdk-store                │──►│ mdk-format    │  │ mdk-markdown  │
  │ scan, read, atomic write │   │ parse / write │  │ commands +    │
  │ path guard, watcher      │   │ cards, quick- │  │ TextView      │
  │ (notify, bg thread)      │   │ add, due fmt  │  │ plugins       │
  └──────────┬───────────────┘   └───────────────┘  └───────────────┘
             │ std::fs                               mdk-vim (pure)
       project folders on disk  ◄── AI agents edit the same files
```

**The keystone: the `Board` entity is the single writer.** Every change to a card file (move, edit, create, delete, archive, undo) goes through one `Board` method. That method:
1. Re-reads the file from disk.
2. Applies only the changed fields with `mdk-format`.
3. Writes atomically through `mdk-store`.
4. Records the result's hash so the watcher ignores the echo.
5. Pushes an inverse onto the `UndoStack`.

Views never touch the disk. The watcher never writes. This one rule is what delivers FR-AGENT-03, FR-EDIT-08, FR-LIVE-02 and FR-MOVE-05.

## Data models

**Conventions:**
- A card's identity inside mdk is `CardKey { project: ProjectId, rel_path: RelPath }`. The frontmatter `id` is data, not a key: Electron lets two projects, or a careless agent, reuse an id.
- `ProjectId` is a random 8-character base36 string, as in Electron.
- Timestamps in files are ISO 8601 UTC strings, kept as strings exactly as found. mdk writes `modified` as `YYYY-MM-DDTHH:MM:SS.sssZ`, same as JavaScript's `toISOString()`.
- Due dates are `YYYY-MM-DD` strings. `order` is `f64`, written without a trailing `.0` when it's a whole number (JSON-style, matching Electron output).
- No soft delete in files. Archive moves the file.
- Single user, no tenancy.

### Project (in memory and in config.json)
| Field  | Type                                       | Notes                                         |
| ------ | ------------------------------------------ | --------------------------------------------- |
| id     | `ProjectId` (String, 8 chars)              | primary key, random base36                    |
| name   | String                                     | folder name at connect time, shown in sidebar |
| root   | `PathBuf`                                  | absolute, canonicalised                       |
| order  | u32                                        | position in sidebar                           |
| status | enum `Ok \| Missing \| Unreadable(String)` | runtime only, FR-PROJ-09                      |

### Card (in memory only; the file is the truth)
| Field        | Type                                                   | Notes                                                                       |
| ------------ | ------------------------------------------------------ | --------------------------------------------------------------------------- |
| key          | `CardKey`                                              | project id plus relative path, unique                                       |
| id           | String                                                 | from frontmatter, or the file stem                                          |
| title        | String                                                 | first `# ` heading, or `id`                                                 |
| body         | String                                                 | text after the frontmatter, trimmed like Electron                           |
| status       | enum `Backlog \| Todo \| InProgress \| Review \| Done` | serialised `backlog` `todo` `in-progress` `review` `done`                   |
| priority     | enum `Critical \| High \| Medium \| Low`               | default Medium                                                              |
| assignee     | String                                                 | empty = none                                                                |
| due_date     | `Option<String>`                                       | `YYYY-MM-DD`                                                                |
| created      | String                                                 | ISO, kept as read                                                           |
| modified     | String                                                 | ISO, set on every mdk write                                                 |
| labels       | `Vec<String>`                                          | order kept                                                                  |
| order        | f64                                                    | sort key within a column                                                    |
| workspace    | String                                                 | first path segment, `""` at the root                                        |
| file_name    | String                                                 |                                                                             |
| extra        | `Vec<RawKey>`                                          | unknown top-level keys: `{ name, source_lines }`, verbatim (FR-FILE-04)     |
| line_ending  | enum `Lf \| CrLf`                                      | kept on write (FR-FILE-08)                                                  |
| parse_error  | `Option<String>`                                       | broken YAML (FR-FILE-05). The card can't be moved, and its raw text is kept |
| raw          | `Arc<str>`                                             | the exact file text last read                                               |
| content_hash | u64                                                    | hash of `raw`, for own-write suppression and change detection               |

Derived on read and cached: `excerpt` (FR-CARD-01), `subtasks: Vec<Subtask{done, text, line}>`, and `search_text` (lowercased title, body, id, assignee and labels joined).

### Column (constant)
| Field | Type | Notes |
|-------|------|-------|
| status | Status | |
| name | &'static str | Backlog, To Do, In Progress, Review, Done |
| color | Hsla | #6b7280, #3b82f6, #f59e0b, #8b5cf6, #22c55e |

### Filters and SavedView
| Field | Type | Notes |
|-------|------|-------|
| query | String | |
| priorities | `Vec<Priority>` | empty = all |
| assignee | String | exact, case-insensitive |
| label | `Option<String>` | |
| due | enum `All \| Overdue \| Today \| Week \| None` | Electron's date rules (Overdue = due date 23:59:59 local time is in the past) |

`SavedView { name: String, filters: Filters }`, stored per project id, with `"all"` for All projects (FR-FILT-02).

### Prefs (config.json)
| Field | Type | Default |
|-------|------|---------|
| version | u32 | 1, for migrations |
| projects | `Vec<Project>` (id, name, root, order) | [] |
| selected_project | `ProjectId \| "all"` | "all" |
| selected_workspace | `String \| "all"` | "all" |
| auto_refresh | bool | true |
| vim_keys | bool | false |
| theme | enum `Dark \| Light \| System` | Dark |
| view | enum `Board \| List` | Board |
| editor_tab | enum `Markdown \| Preview` | Markdown |
| editor_expanded | bool | false |
| saved_views | `Map<String, Vec<SavedView>>` | {} |
| window | `{ x, y, w, h, maximized, display_id }` | centred 1440x900 |

### UndoEntry (in memory, per session)
| Field | Type | Notes |
|-------|------|-------|
| label | String | "Move to Done", "Delete Fix login" |
| ops | `Vec<FileOp>` | `Write{key, before: Option<bytes>, after: bytes}`, `Rename{from, to}`, `Delete{key, bytes}` |

Undo applies the inverse ops. If a file changed on disk since the op (hash mismatch), undo for that entry is refused with a toast "Changed on disk since, can't undo".

### StoreEvent (watcher to Board)
`Changed { project, rel_path }`, `Removed { project, rel_path }`, `Renamed { project, from, to }`, `Rescan { project }` (overflow or a burst), `RootMissing { project }`, `Error { project, message }`.

## Command surface

Style: plain Rust methods on the `Board` entity (`impl Board`) plus GPUI `actions!` for keyboard shortcuts and menus. Errors use a single `MdkError` enum (`Io`, `OutsideRoot`, `Parse`, `Conflict`, `NotFound`, `BrokenFrontmatter`), shown in the UI as toasts with a plain message. No auth: single user. File operations run on the background executor, and the UI updates when they finish.

### Projects
- **`connect_project(path) -> Result<ProjectId>`** : canonicalise, refuse duplicates, scan, start watching, select (FR-PROJ-01, FR-PROJ-02).
- **`remove_project(id)`** : stop watching, forget. Never touches files (FR-PROJ-07).
- **`relocate_project(id, new_path)`** : for a missing folder (FR-PROJ-09).
- **`rescan(id) / rescan_all()`** : full re-read, keeping old cards if the read fails (FR-PROJ-08).
- **`select_project(id | All) / select_workspace(name | All)`** (FR-PROJ-04, FR-PROJ-05).
- **`create_sample_project(parent_dir) -> Result<ProjectId>`** (FR-FIRST-02).

### Cards
- **`create_card(input: NewCard, workspace) -> Result<CardKey>`** : slug-date file name, de-duplicated with a suffix (FR-NEW-04, FR-FILE-06).
- **`update_card(key, patch: CardPatch) -> Result<()>`** : `CardPatch` has an `Option` per field. Re-read, merge, write. Title changes rewrite the `# ` heading (FR-EDIT-02, FR-EDIT-03, FR-AGENT-03).
- **`move_card(key, status, index: Option<usize>) -> Result<()>`** : Electron's `moveTaskAt` order maths, with renumbering when values collide (FR-MOVE-01, FR-MOVE-02).
- **`delete_card(key) -> Result<()>`** : keeps the bytes for undo (FR-EDIT-06).
- **`archive_done(scope) -> Result<usize>`** : rename into `archive/` with suffixing (FR-BOARD-05).
- **`toggle_subtask(key, index) -> Result<()>`** (FR-PREV-02).
- **`undo() / redo()`** (FR-MOVE-05).
- **`resolve_conflict(key, KeepMine | LoadTheirs)`** (FR-EDIT-08).

### Queries (pure, on the in-memory state)
- **`visible_cards() -> Vec<&Card>`** : selection, then filters, then sorted by order (FR-FILT-01).
- **`column(status) -> Vec<&Card>`**, **`workspaces() -> Vec<String>`**, **`labels_in_view() -> Vec<String>`**, **`open_counts() -> Map<ProjectId, usize>`** (FR-PROJ-03).
- **`palette_items(query, open_card) -> Vec<PaletteItem>`** (FR-PAL-01, FR-PAL-02).
- **`resolve_link(from: &CardKey, target: &str) -> Option<CardKey>`** : relative links and wikilinks by file name, path, id or title (FR-PREV-08).
- **`read_image(key, rel_path) -> Result<Arc<Image>>`** : image types only, inside the root (FR-PREV-09, FR-FILE-09).

### mdk-format (pure functions)
- `parse(raw: &str, rel_path) -> ParsedCard` (FR-FILE-02, FR-FILE-05).
- `write(card: &ParsedCard) -> String` (FR-FILE-03, FR-FILE-04).
- `apply_patch(raw: &str, patch) -> Result<String>` : re-read-merge-write in one step.
- `parse_quick_add(input, today) -> QuickAdd` (FR-NEW-02), and `template(kind, title)` (FR-NEW-03).
- `format_due(due, today) -> (label, Tone)` (FR-CARD-01).
- `strip_comments(md) -> String` (FR-PREV-07).
- `subtasks(body) / toggle_subtask(body, i)` (FR-PREV-02).
- `slugify(title)`, `new_file_name(title, date)` (FR-FILE-06).

### mdk-markdown
- `commands::{wrap, link, heading, toggle_list, code_block, table, callout, math_block, mermaid, rule}(text, selections) -> Edit` (FR-MDE-02).
- Plugins: `CalloutPlugin`, `WikiLinkPlugin`, `HighlightPlugin`, `MathPlugin`, `MermaidPlugin`, `CodeBlockPlugin`, `ImagePlugin`, `RawHtmlPlugin` (FR-PREV-03 to FR-PREV-10).
- `dollar_guard(md) -> md` (FR-PREV-04).

### mdk-vim
- `Vim::handle(&mut self, key: Keystroke, buf: &mut dyn VimBuffer) -> Handled`, where `VimBuffer` is a small trait (text, cursor, selection, replace range, undo, redo, search) that the editor adapter implements and the tests implement on a `String` (FR-VIM-02, FR-VIM-03).

### Actions (keyboard and menu)
`NewCard (N)`, `TogglePalette (cmd-k)`, `Undo (cmd-z)`, `Redo (cmd-shift-z)`, `CloseTop (escape)`, `OpenSettings (cmd-,)`, `ConnectFolder`, `RescanAll`, `ToggleTheme`, `ToggleVim`, `ShowBoard`, `ShowList`, `ArchiveDone`, plus card focus actions `PickUp (space)`, `MoveLeft`, `MoveRight`, `MoveUp`, `MoveDown`, `Drop`, `CancelMove`. `secondary-` maps to Ctrl on Windows and Linux (FR-APP-05, FR-MOVE-04).

## Permissions
Single user. The only authorisation is the path guard: every path is resolved against the project root, and `OutsideRoot` is returned for anything outside it (FR-FILE-09). Image reads check the extension against `png jpg jpeg gif webp avif svg bmp ico`.

## Screens and key components

### Main window
- **Purpose:** the board (FR-APP-01, FR-BOARD-*).
- **Components:**
  - **`Header`**: app mark and "mdk", the stats line, the project chip, "Search ⌘K" button, theme toggle, and primary "New card N".
  - **`Sidebar`**, 240 px (FR-PROJ-*, FR-LIVE-03, FR-VIM-01):
    - Connect folder button.
    - PROJECTS: All projects, then each project with its count. The selected project shows rescan and remove, or "Locate folder" when missing.
    - WORK FOLDERS (monospace): all, then the folders.
    - Footer: Rescan all, Auto-refresh switch, Vim keys switch, "checked HH:MM:SS".
    - Below 1024 px the sidebar is hidden behind a header button.
  - **`FilterBar`** (FR-FILT-01, FR-FILT-02, FR-FILT-03): search input, four toggle chips, assignee input, labels select, due select, reset link, filtered/all badge, "save view" link, a row of saved-view tags with ×, and the Board/List segmented control.
  - **`BoardView`** (FR-BOARD-*, FR-MOVE-*): a horizontal scroll of five `ColumnView`s, 288 px wide. Each has a header (dot, name, count badge, "archive" on Done, "+"), a vertical list of `CardView`s (virtualised past 100 cards), drop-slot elements, and "+ Add card".
  - **`CardView`** (FR-CARD-*): focusable, draggable, with the move menu popover and the picked-up state for keyboard moves.
  - **`ListView`** (FR-FILT-03, FR-FILT-04): a data-table with Title, Status, Priority, Due and Assignee, sortable.
- **States:**
  - Loading: "Loading folders..." on the first scan only.
  - Empty, no projects: the welcome state with Connect folder and Create sample project (FR-FIRST-01).
  - Empty, nothing matches: "No cards match" with "Create first card" (FR-FIRST-03).
  - Error: a project error row in the sidebar, plus toasts.
  - Success: the board.

### Card editor sheet
- **Purpose:** view and edit one card (FR-EDIT-*).
- **Components:**
  - Header: the path line, title input and close X.
  - Field grid: status select, priority select, assignee input, due date picker, labels input.
  - A clash banner (FR-EDIT-08) or a deleted banner (FR-EDIT-09) when needed.
  - Tabs (Markdown, Preview) and an Expand toggle.
  - **Markdown tab:** the formatting toolbar (20 buttons in 5 groups with separators and tooltips), the `Editor`, and the Vim mode line when Vim is on.
  - **Preview tab:** the subtasks box with checkboxes, then the TextView with the mdk plugins.
  - Footer: "Copy AI prompt", "Autosaves to the markdown file.", and Delete (destructive).
  - A collapsible raw file view.
- **States:** clean, saving (a subtle dot), clash, deleted, broken frontmatter (fields read-only, with a banner saying "Fix the YAML in the Markdown tab", where the full raw file is editable).

### New card dialog
- **Purpose:** FR-NEW-*.
- **Components:** title input with the quick-add hint line, template buttons (bug, feature, chore), description textarea, then a two-column grid (status, priority, assignee, due date picker, labels, work subfolder), Cancel and Create card, and the "Tip: press N" line.
- **States:** invalid (empty title disables Create), creating, error toast on write failure.

### Command palette
- **Purpose:** FR-PAL-*.
- **Components:** gpui-kit `command`: input, up to 8 card results with a status hint, "Create card ...", "Move open card to ..." and app commands, plus a footer of key hints.
- **States:** empty query shows the first 8 cards. No results shows "No matches".

### Settings dialog (Should)
FR-APP-07: theme radio, auto-refresh switch, Vim switch, and "Open config folder".

### Toasts
gpui-kit notification, bottom centre for "Moved to X · Undo" (5 s), and for errors and "Copied".

## State management
- **Source of truth:** card files on disk. In memory, the `Board` entity holds `projects`, `cards: HashMap<CardKey, Card>`, `selection`, `filters`, `undo`, `pending_writes`, and the open editor's dirty state.
- **Views** observe `Board` (`cx.observe`) and re-render on `cx.notify()`. Derived lists (`visible_cards`, columns) are recomputed when the board version changes, and cached by version number.
- **Editor sheet** keeps a local draft (title, fields, body). A 500 ms debounce calls `update_card`. `flush()` runs before switching cards and on close (FR-EDIT-02). `dirty = draft != last saved` drives FR-EDIT-08.
- **Watcher events** run `on_store_event`:
  - If the hash equals the last mdk write, ignore it.
  - Otherwise re-parse that one file and replace the card.
  - If it's the card open in the editor: reload when clean, or show the clash banner when dirty.
- **Undo** belongs to the Board, and is per session.
- **Persistence:** Prefs writes `config.json` atomically, 300 ms after any change, and on quit.

## Background and long-running tasks
- **Initial scan per project:** background executor. Results are merged into the Board. Projects scan in parallel. Failure marks the project `Unreadable` and keeps its old cards.
- **Watcher:** one `notify` debouncer per project on a background thread, with a 150 ms debounce. A burst of more than 200 events in one batch is collapsed into `Rescan { project }`. The root folder disappearing gives `RootMissing`.
- **Writes:** background executor, serialised per file so two writes to one file never overlap. Failure shows an error toast and keeps the draft dirty for a retry.
- **Preview rendering:** markdown parsing runs on TextView's background task. Mermaid SVG and maths layout run on the background executor, cached by `(source hash, theme)` in an LRU of 64 entries.
- **Image loading:** background, cached per `(project, rel_path, mtime)`.

## Security
- Path guard on every file operation (FR-FILE-09). Symlinks are resolved and must stay inside the root.
- No network: no HTTP client in the dependency tree. CI checks that `reqwest`, `ureq` and `hyper` are absent with `cargo-deny` bans (NFR-02).
- External links open through the OS (`cx.open_url`) only for `http`, `https` and `mailto` schemes. `file:` and `javascript:` links are refused.
- Markdown is never executed. Raw HTML is shown as text (FR-PREV-10).
- YAML parsing is panic-free (serde-saphyr), with a 1 MB file size cap per card (NFR-04). Bigger files are listed with a "too large" marker and not parsed.
- Config contains only paths and preferences. No secrets exist anywhere.

## Logging and observability
- `log` plus `env_logger`, at info level by default, written to `mdk.log` in the OS log or config folder, rotated at 5 MB, keeping 2 files.
- Logged: app start (version, OS), project connect and remove, scan durations and card counts, watcher errors, write failures, and parse errors (file path and error).
- Never logged: card bodies or frontmatter values.
- `MDK_LOG=debug` raises the level. Panics are caught with a panic hook that writes the backtrace to the log.

## Testing strategy
- **`mdk-format` unit and golden tests:**
  - Every `.md` in the Electron repo's `tasks/` and `examples/` folders is parsed, and the result is compared to JSON fixtures generated once from Electron's `parseTaskFile` (FR-FILE-02).
  - Round-trip tests (FR-FILE-03, FR-FILE-04, FR-FILE-08).
  - The quick-add cases from Electron, plus the Sunday 2026-10-11 example (FR-NEW-02).
  - The due label cases (FR-CARD-01).
- **`mdk-store` integration tests** in temp folders: atomic write under a simulated failure; the path guard with `..` and symlinks; the watcher picking up an external edit in under 500 ms; a 50-file burst producing one refresh; own-write suppression; rename handling. These run on all three CI OSes (FR-FILE-07, FR-FILE-09, FR-LIVE-*).
- **`mdk-markdown`:** port Electron's `md-commands.check.ts` and `render.check.ts` cases. Plugin parse tests for callouts, wikilinks, highlight, the dollar guard and comments (FR-MDE-02, FR-PREV-*).
- **`mdk-vim`:** a table of `(start text, cursor, keys, expected text, expected cursor)` for every FR-VIM-02 command.
- **App tests** with GPUI's `TestAppContext`, headless: move, undo and redo, the clash banner, palette flows, keyboard card moving, new card with quick-add, archive with confirm.
- **Performance script** (`scripts/bench.sh`): generates a 1,000-card project, measures cold start, idle RSS after 10 minutes, and edit-to-screen latency (a timestamp written into a card, compared with the frame where it appears), for mdk and for Electron (NFR-01).
- **Manual release checklist** on each OS: install, first run, sample project, drag, editor and Vim, preview with maths and Mermaid, and contrast check in both themes (NFR-06).
- **Run:** `cargo test --workspace`, and `cargo run -p mdk` for the app.

## Environment and configuration
- `MDK_LOG` : log level, default `info`.
- `MDK_CONFIG_DIR` : override the config folder (tests and portable use). Default is the OS config folder.
- `MDK_NO_WATCH` : set to `1` to disable the watcher, for debugging.

No secrets, no API keys, nothing in the repo that differs per machine.

## Migrations and seed data
- `config.json` has `version: 1`. Future versions migrate on load, with a backup copy `config.v1.json`. An unreadable config is renamed `config.broken-<timestamp>.json`, and mdk starts fresh with a toast.
- Card files are never migrated: the format is the Electron format, unchanged.
- **Seed data:** the sample project (FR-FIRST-02) is embedded in the binary with `include_str!`. It is built from the Electron repo's `examples/demo-project/` plus new cards covering every preview feature.
- **Importing existing Electron projects:** nothing to import. The user connects the same folders. (Electron's project list lives in its own store and is not read.)

---

### Open questions
None.

### Assumptions confirmed
1. **Card identity : RESOLVED.** `project id + relative path`, not the frontmatter `id`, because ids can repeat.
2. **Single writer : RESOLVED.** Only the Board writes files: re-read, merge, then write atomically.
3. **Burst handling : RESOLVED.** 150 ms debounce. More than 200 events becomes one project rescan.
