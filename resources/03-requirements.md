# Functional requirements

Status: signed off 2026-10-11

Requirements say what mdk must do, not how. Tech choices are in Stage 4. Each traces to the product (Stage 1) and the journeys and jobs (Stage 2).

**Parity source:** the Electron app at github.com/Abdulkader-Safi/md-kanban, commit `edebd29` (10 Sep 2026). "Same as Electron" below means the behaviour in that commit. Where mdk deliberately differs, the requirement says so and why.

Priorities: **Must** = v1 doesn't ship without it. **Should** = planned for v1, can slip to v1.1. **Could** = only if cheap. **Won't (v1)** = listed under out of scope.

## Core features (MVP)

### Projects and folders (PROJ)
- **FR-PROJ-01 [Must]** The user can connect a folder from disk as a project, through the OS folder picker, from the sidebar "Connect folder" button.
  *Acceptance:* clicking Connect folder opens the native picker. Choosing `~/code/app/tasks` adds a project named `tasks` to the sidebar, selects it, and shows its cards.
- **FR-PROJ-02 [Must]** The user can connect several folders. Connected projects and their order survive a restart.
  *Acceptance:* connect two folders, quit, relaunch: both are listed and the board shows both projects' cards.
- **FR-PROJ-03 [Must]** The sidebar lists "All projects" plus each project with its count of cards that are not done, same as Electron.
  *Acceptance:* a project with 37 cards, 10 of them done, shows `27`.
- **FR-PROJ-04 [Must]** Selecting a project filters the board to it. "All projects" shows every project's cards. The selection survives a restart.
- **FR-PROJ-05 [Must]** The first-level subfolders of a project are its work folders. The sidebar lists them under WORK FOLDERS (with "all" first), and selecting one filters the board to it. With "All projects" selected, the list shows work folders from every project.
  *Acceptance:* `tasks/backend/x.md` and `tasks/board/y.md` show work folders `backend` and `board`. Selecting `board` shows only `y`.
- **FR-PROJ-06 [Must]** The `archive` work folder is hidden from the board unless the user selects it, same as Electron.
- **FR-PROJ-07 [Must]** A selected project shows "rescan" and "remove" actions. Remove disconnects the project from mdk and never deletes files.
  *Acceptance:* remove a project: it leaves the sidebar, its files on disk are unchanged.
- **FR-PROJ-08 [Must]** "Rescan all" re-reads every project from disk.
- **FR-PROJ-09 [Must]** If a project folder is missing or unreadable (moved, deleted, drive unplugged), the project stays in the sidebar with an error state and a "Locate folder" action. The last cards read are shown, marked as stale.
  *Acceptance:* rename a connected folder in Finder: the project shows "Folder not found" and Locate folder. Pointing it at the new path restores it.
- **FR-PROJ-10 [Should]** Dropping a folder from the OS onto the window connects it as a project.

### Card files and format (FILE)
- **FR-FILE-01 [Must]** mdk reads every `.md` file under a project folder, up to 4 levels deep, skipping hidden entries (names starting with `.`) and the folders `node_modules`, `dist`, `out` and `.git`. Same rule as Electron: every `.md` file is a card, including `README.md` or `AGENTS.md` if they sit in the folder.
- **FR-FILE-02 [Must]** mdk reads the frontmatter fields `id`, `status`, `priority`, `assignee`, `dueDate`, `created`, `modified`, `labels` and `order`, with the same defaults as Electron: missing `id` = file name without `.md`; unknown or missing `status` = `backlog`; unknown or missing `priority` = `medium`; missing `order` = 0. The title is the body's first `# ` heading, or the id.
  *Acceptance:* the parser test suite runs every file in the Electron repo's `tasks/` and `examples/` folders, and produces the same field values as Electron's `parseTaskFile`.
- **FR-FILE-03 [Must]** mdk writes cards in the Electron format: the nine keys in the same order, strings double-quoted, `dueDate: null` when empty, `labels` as a one-line list, `modified` set to the current UTC ISO time, and a `# Title` heading first in the body. A file written by mdk must parse identically in the Electron app, and the other way round.
  *Acceptance:* round-trip test: parse then write each sample file. The nine keys and the body come out the same (except `modified`).
- **FR-FILE-04 [Must]** **Unknown frontmatter keys are kept** when mdk saves a card, in their original order, after the nine known keys. This fixes the Electron bug where `stringifyTaskFile` drops them.
  *Acceptance:* a card with `sprint: 4` and `agent: "claude"` keeps both keys after being dragged to another column.
- **FR-FILE-05 [Must]** A file with broken YAML is still shown as a card (title from the body, defaults for fields) with a "frontmatter error" marker, and mdk never rewrites it until the user edits it in the card editor. Its raw text is never lost.
  *Acceptance:* a file with `status: [todo` shows on the board with a warning icon. Dragging it is blocked, with a toast saying "Fix the frontmatter first".
- **FR-FILE-06 [Must]** New card files are named `<slug>-YYYY-MM-DD.md` with id `<slug>-YYYY-MM-DD`. They are written in the selected work folder (or the project root), same as Electron. If the name is taken, `-2`, `-3` and so on are appended.
- **FR-FILE-07 [Must]** File writes are atomic: write to a temp file in the same folder, then rename over the original. A crash or power cut never leaves a half-written card.
- **FR-FILE-08 [Must]** Files are read and written as UTF-8. A leading BOM is ignored on read. Line endings are kept as found (CRLF stays CRLF).
- **FR-FILE-09 [Must]** mdk never reads or writes outside a connected project folder. Paths with `..` that climb out are refused. Image reads are limited to image file types.

### Board (BOARD)
- **FR-BOARD-01 [Must]** The board shows five fixed columns in order, with Electron's names and colours: Backlog `#6b7280`, To Do `#3b82f6`, In Progress `#f59e0b`, Review `#8b5cf6`, Done `#22c55e`.
- **FR-BOARD-02 [Must]** Each column header shows a colour dot, the name, the card count, and a "+" that opens New card with that status. Each column ends with a dashed "+ Add card" button.
- **FR-BOARD-03 [Must]** Cards in a column are sorted by `order`, lowest first.
- **FR-BOARD-04 [Must]** The board scrolls horizontally when columns don't fit. Each column scrolls vertically on its own. The board does not scroll vertically past its content (the Electron overscroll bug stays fixed).
- **FR-BOARD-05 [Must]** The Done column has an "archive" action that moves every visible done card into the project's `archive/` folder (same file content, new path; a numeric suffix if the name is taken), after a confirmation dialog stating how many cards will move.
  *Acceptance:* 12 done cards, click archive, confirm: 12 files now live under `archive/`, the Done column is empty, and `git status` shows 12 renames.
- **FR-BOARD-06 [Must]** Columns render a few hundred cards without stutter (see NFR-01).

### Card face (CARD)
- **FR-CARD-01 [Must]** A card shows, same as Electron: title (bold, wrapping), priority badge (critical red, high orange, medium blue, low grey), a "⋯" move menu, an excerpt (body without the title heading and without `%% comments %%`, first 140 characters, two lines max), then chips for `@assignee`, due ("Today", "Tomorrow", "3d", "2d overdue", "2026-10-11" followed by "· YYYY-MM-DD"; red when overdue, amber today), work folder in monospace, and subtask progress "☑ done/total" when the body has checkboxes. Then up to three `#labels` plus "+N more", then the file path in small monospace.
  *Acceptance:* visual comparison with `references/electron-board-screenshot.png`, using the same card files.
- **FR-CARD-02 [Must]** Clicking a card opens it in the card editor (EDIT).
- **FR-CARD-03 [Must]** The "⋯" menu lists the five columns, with the current one disabled. Choosing one moves the card to the end of that column.
- **FR-CARD-04 [Must]** A card whose file has a frontmatter error shows a warning marker with the error on hover (FR-FILE-05).

### Moving cards (MOVE)
- **FR-MOVE-01 [Must]** The user can drag a card to another column or to a new position in the same column. A drop slot shows where it will land. The column under the pointer highlights. The column list auto-scrolls near its top or bottom edge, and the board auto-scrolls near the left or right edge.
  *Acceptance:* drag a card from Backlog to between the 2nd and 3rd cards of Review: it lands there, and its file's `status` is `review` with an `order` between its neighbours.
- **FR-MOVE-02 [Must]** A move rewrites only that card's `status`, `order` and `modified`. If order values collide, the column is renumbered (0, 10, 20 and so on), same as Electron.
- **FR-MOVE-03 [Must]** After any move, a toast "Moved to <column>" with an Undo button shows for 5 seconds. Undo puts the card back to its previous status and order.
  *Acceptance:* drag a card to Done, click Undo within 5 s: the card is back in its old place and the file's `status` is restored.
- **FR-MOVE-04 [Must]** Keyboard moving, same as Electron: with a card focused, Space picks it up, Left and Right move it between columns, Up and Down move it within the column, Space drops it, Esc cancels and restores its original place. While a card is picked up, it shows "Moving: arrows shift, Space drops, Esc cancels".
- **FR-MOVE-05 [Must]** Undo and redo for board actions: ⌘Z / Ctrl+Z undoes the last move, archive, create or delete made in this session, and ⌘⇧Z / Ctrl+Shift+Z redoes it. Undo of a delete restores the file with its exact original content. (New in mdk. Electron only had the 5-second toast.)
  *Acceptance:* delete a card, press ⌘Z: the file is back, byte for byte.

### Creating cards (NEW)
- **FR-NEW-01 [Must]** "New card" (header button, `N` when not typing, column "+", "+ Add card") opens a New card dialog with: title, description (markdown), status, priority, assignee, due date (date picker), labels (comma separated) and work subfolder. Defaults: the column's status, medium priority, and the selected work folder.
- **FR-NEW-02 [Must]** Quick-add syntax in the title, same as Electron: `p1`..`p4` or `critical`/`high`/`medium`/`low` set priority, `#tag` adds a label, `@name` sets assignee, `due:today|tomorrow|<weekday>|YYYY-MM-DD` sets the due date. A live hint line shows "Will set: high · @safi · due 2026-10-16 · #backend".
  *Acceptance:* typing `Fix login p1 #backend @safi due:friday` on Sunday 2026-10-11 creates a card titled "Fix login", critical, label backend, assignee safi, due 2026-10-16.
- **FR-NEW-03 [Must]** Template buttons bug, feature and chore fill priority, labels and body with Electron's templates.
- **FR-NEW-04 [Must]** Creating a card writes the file (FR-FILE-06) and the card appears in its column at once.
- **FR-NEW-05 [Must]** The dialog closes on Cancel, Esc, or a click that both starts and ends on the backdrop (so drag-selecting text out of the dialog never closes it).

### Card editor (EDIT)
- **FR-EDIT-01 [Must]** The card editor is a right-hand side sheet over the board (672 px wide, "Expand" makes it two-thirds of the window), same as Electron. It shows: `project/relPath` in monospace, an editable title, a grid with status, priority, assignee and due date (date picker), and a labels field.
- **FR-EDIT-02 [Must]** Edits autosave 500 ms after the last change. A pending save is flushed before the editor switches cards or closes, so no edit is lost or written to the wrong file.
  *Acceptance:* type in the body and close the sheet within 100 ms: the file contains the text.
- **FR-EDIT-03 [Must]** Changing the title also changes the body's first `# ` heading.
- **FR-EDIT-04 [Must]** Markdown and Preview tabs. The last tab used is remembered across restarts.
- **FR-EDIT-05 [Must]** "Copy AI prompt" copies Electron's prompt text (file path, title, status, priority, assignee, labels, body and the working rules) to the clipboard and shows a "Copied" toast.
- **FR-EDIT-06 [Must]** Delete asks for confirmation ("Delete <title>? This removes the markdown file."), then deletes the file and closes the sheet. Undoable (FR-MOVE-05).
- **FR-EDIT-07 [Must]** A collapsible "Raw markdown file" section shows the exact text that will be written.
- **FR-EDIT-08 [Must]** **Edit clash handling.** If the open card's file changes on disk while the editor is open: when there are no unsaved edits, the editor reloads the new content silently. When there are unsaved edits, a banner says "Changed on disk" with "Keep mine" (overwrite with your version) and "Load theirs" (discard your edits). mdk never silently overwrites an agent's change.
  *Acceptance:* open a card, type without waiting for autosave, then change its file from a script: the banner appears. With no typing, the editor shows the script's change within 500 ms.
- **FR-EDIT-09 [Must]** If the open card's file is deleted on disk, the editor shows "This file was deleted" with "Recreate" and "Close".
- **FR-EDIT-10 [Must]** The sheet closes on the X, Esc (unless Vim mode has taken Esc), or a click that both starts and ends on the backdrop.

### Markdown editor (MDE)
- **FR-MDE-01 [Must]** A plain-text markdown editor with markdown syntax highlighting (headings bigger and bold, bold, italic, strikethrough, links coloured, inline code coloured, markup characters dimmed, fenced code highlighted by language), line wrapping, an active-line highlight, selection-match highlighting, and a "Write markdown..." placeholder. Colours follow the theme.
- **FR-MDE-02 [Must]** A formatting toolbar with Electron's 20 actions, in groups: H1, H2, H3 | bold, italic, strikethrough, highlight `==`, inline code | link, image | bulleted list, numbered list, checklist, quote | code block, table, callout, math block, Mermaid diagram, divider. Each has a tooltip. Behaviour matches Electron's `md-commands.ts` (wrap toggles marks on and off; an empty selection inserts a placeholder and selects it; list commands toggle line prefixes over the selected lines).
  *Acceptance:* port Electron's `md-commands.check.ts` cases as tests. All pass.
- **FR-MDE-03 [Must]** Shortcuts: ⌘B bold, ⌘I italic, ⌘E inline code (Ctrl on Windows and Linux), ⌘F find in the card, ⌘Z/⌘⇧Z editor undo and redo, Tab and Shift-Tab indent and outdent list items, Enter continues lists, checklists and quotes.
- **FR-MDE-04 [Must]** Standard text editing: mouse and keyboard selection, multi-line selection, copy, cut, paste, select all, word and line navigation, and IME input for non-Latin keyboards.

### Vim mode (VIM)
- **FR-VIM-01 [Must]** A "Vim keys in editor" toggle in the sidebar footer turns Vim mode on or off for the card editor only. The setting persists.
- **FR-VIM-02 [Must]** Vim subset: normal, insert and visual (charwise and linewise) modes, with a mode indicator under the editor. Motions `h j k l w b e 0 ^ $ gg G`, with counts (`3j`, `2w`). Insert with `i a I A o O`. Edit with `x dd yy p P u Ctrl-r`. Operators `d c y` with any of the motions above (`dw`, `c$`, `y2j`). In visual mode, `d y c > <`. Search with `/` then `n N`. `:w` does nothing (cards autosave) and `:q` closes the card.
  *Acceptance:* a scripted test drives each command on a sample buffer and checks the resulting text and cursor position.
- **FR-VIM-03 [Must]** With Vim on, Esc returns to normal mode and never closes the card sheet, same as Electron.
- **FR-VIM-04 [Won't (v1)]** Registers, macros, marks, text objects (`ciw`, `da"`), and other ex commands. See out of scope.

### Preview (PREV)
- **FR-PREV-01 [Must]** The Preview tab renders the card body as GitHub-flavoured markdown: headings, paragraphs, emphasis, strikethrough, links, lists, task lists, blockquotes, tables (scroll sideways when wide), horizontal rules, footnotes, inline code and fenced code.
- **FR-PREV-02 [Must]** Above the rendered body, a "Subtasks done/total" box lists every checklist item with a checkbox. Ticking one flips that `- [ ]` in the body and autosaves.
  *Acceptance:* tick the 2nd of 5 subtasks: line 2's `[ ]` becomes `[x]` in the file. The card face shows ☑ 1/5 (or one more than before).
- **FR-PREV-03 [Must]** Fenced code is syntax highlighted. A header shows the language, or the `title="..."` from the fence, plus a Copy button. Unknown languages render as plain text.
- **FR-PREV-04 [Must]** Maths: `$inline$` and `$$block$$` render as typeset maths. The Electron "dollar guard" rule applies: inline maths cannot start or end with a space, and a closing `$` followed by a digit is not maths, so "costs $5 a month, or $50" stays text. A formula that fails to parse shows its source in red.
- **FR-PREV-05 [Must]** Mermaid: ` ```mermaid ` fences render as diagrams in the theme's colours. A diagram that fails to render shows its source and the error.
- **FR-PREV-06 [Must]** Callouts in GitHub style (`> [!NOTE]`, TIP, IMPORTANT, WARNING, CAUTION) and Obsidian style (`> [!tip] Custom title`, `> [!faq]-` starts folded, `> [!faq]+` starts open), mapped to five looks as in Electron. Unknown types render as a note. Folded callouts open and close on click.
- **FR-PREV-07 [Must]** Obsidian extras: `==highlight==` (not when the text inside starts or ends with a space), and `%% comments %%` hidden in the preview and on the card face, but kept in the file. Fenced code keeps its `%%`.
- **FR-PREV-08 [Must]** Links: external `http(s)` links open in the default browser. Relative links to another card's file, and wikilinks `[[card]]`, `[[card|label]]`, `[[card#Heading]]` (matched by file name, path, id or title, within the same project), open that card in the editor. Links that climb out of the project are shown as plain text. Unresolved wikilinks are shown dimmed.
- **FR-PREV-09 [Must]** Images: `![alt](path)` and `![[image.png]]`, `![[image.png|300]]` (width) load from the project folder. A missing image shows its name in a box. Remote `http(s)` images are not loaded (no network, see NFR-03) and show as a link.
- **FR-PREV-10 [Must]** Raw HTML in markdown is not rendered. It is shown as text, except for `<br>`, which becomes a line break. (Electron rendered sanitised HTML. Deliberate difference: a native renderer has no HTML engine.)
- **FR-PREV-11 [Should]** Text in the preview can be selected and copied.

### Filters, views and list (FILT)
- **FR-FILT-01 [Must]** A filter bar above the board with: a search box (matches title, body, id, assignee and labels, case-insensitive), four priority toggle chips (any combination), an assignee text filter, a labels dropdown (labels of the cards in view), a due dropdown (any, overdue, today, this week, no date), "reset", and a badge showing "filtered" or "all". Same as Electron.
  *Acceptance:* choose "high" plus due "overdue": only overdue high-priority cards show, in every column.
- **FR-FILT-02 [Must]** Saved views: "save view" asks for a name and stores the current filters for the selected project. Saved views show as chips. Clicking one applies it, and its × deletes it. Views persist across restarts.
- **FR-FILT-03 [Must]** A Board/List segmented control. List view shows the filtered cards as a table with Title, Status, Priority, Due and Assignee. Clicking or pressing Enter on a row opens the card. The chosen view persists.
- **FR-FILT-04 [Should]** List view columns can be sorted by clicking a header. (New in mdk.)

### Command palette (PAL)
- **FR-PAL-01 [Must]** ⌘K / Ctrl+K toggles a command palette, same as Electron: a search box, up to 8 matching cards (title, body, id, assignee and labels), "Create card <query>" when there is text, and, when a card is open, "Move open card to <column>" for the other four columns. Up and Down select, Enter runs, Esc closes.
  *Acceptance:* ⌘K, type "login", Enter: the login card opens.
- **FR-PAL-02 [Should]** The palette also lists app commands: Connect folder, Rescan all, Toggle theme, Toggle Vim keys, Board view, List view, Archive done cards. (New in mdk.)

### Live updates (LIVE)
- **FR-LIVE-01 [Must]** mdk watches every connected project folder and shows card files that are added, changed, deleted or renamed by other programs (agents, git, editors) without any user action. Bursts of changes (a git checkout touching 50 files) settle into one refresh.
  *Acceptance:* a script changes `status: "todo"` to `"done"` in a card file: the card moves to Done within 500 ms. A `git checkout` that changes 50 cards produces one refresh, and the UI doesn't freeze.
- **FR-LIVE-02 [Must]** mdk's own writes don't trigger a pointless reload or a "changed on disk" banner.
- **FR-LIVE-03 [Must]** The sidebar "Auto-refresh folders" toggle pauses and resumes live updates, and persists. The footer shows "checked <time>" with the last refresh time.
- **FR-LIVE-04 [Must]** New subfolders created inside a project are picked up and watched automatically.

### App shell (APP)
- **FR-APP-01 [Must]** Header, same as Electron: app mark and name ("mdk"), "local markdown board · N cards · N projects", the active project's name chip, a "Search ⌘K" button, a theme toggle, and a primary "New card N" button.
- **FR-APP-02 [Must]** Dark and light themes using Electron's colours (dark by default, teal accent). The choice persists. A "System" option follows the OS setting.
- **FR-APP-03 [Must]** Preferences persist across restarts: connected projects, selected project and work folder, auto-refresh, Vim keys, theme, board or list view, editor tab, and saved views.
- **FR-APP-04 [Must]** Window size, position and maximised state are restored on launch.
- **FR-APP-05 [Must]** Global shortcuts: `N` new card (when not typing), ⌘K palette, Esc closes the top dialog, sheet or palette, ⌘Z/⌘⇧Z undo and redo of board actions (FR-MOVE-05), and ⌘, opens Settings (FR-APP-07). Ctrl on Windows and Linux.
- **FR-APP-06 [Must]** A native app menu on macOS (mdk, File, Edit, View, Window, Help) with the main commands. An in-window menu on Windows and Linux.
- **FR-APP-07 [Should]** A small Settings dialog with the same toggles as the sidebar footer (auto-refresh, Vim keys, theme) and an "Open config folder" link.
- **FR-APP-08 [Must]** Only one mdk window. Launching mdk again focuses the open window.

### First run and empty states (FIRST)
- **FR-FIRST-01 [Must]** With no projects connected, the board area shows a welcome state: "Connect a folder" and "Create sample project", plus one line explaining that cards are markdown files your agent can edit.
- **FR-FIRST-02 [Must]** "Create sample project" asks where to save, then writes a `mdk-sample/` folder with 6 to 8 cards (spread over the five columns, with subtasks, labels, a due date, a code block, a callout, maths and a Mermaid diagram), plus a copy of `AGENTS.md`, and connects it.
  *Acceptance:* on a clean install, click it and pick Desktop: `~/Desktop/mdk-sample/` exists, and the board shows its cards in all five columns.
- **FR-FIRST-03 [Must]** When filters or selection hide every card, the board shows "No cards match" with "Create first card", same as Electron.

### Agent contract (AGENT)
- **FR-AGENT-01 [Must]** The file format and rules in the Electron repo's `AGENTS.md` stay valid unchanged. mdk's repo ships the same `AGENTS.md` (format, statuses, priorities, id rule, order rule, allowed operations, status-check recipe) plus a Claude skill at `skills/mdk/SKILL.md`.
- **FR-AGENT-02 [Must]** mdk creates no lock files, databases, caches or hidden files inside project folders. All app state lives in the OS config folder.
  *Acceptance:* after a session of creating, moving and deleting cards, `git status` in the project shows only card `.md` changes.
- **FR-AGENT-03 [Must]** An agent's edit is never overwritten by stale app state: mdk writes only fields the user changed, on top of the latest file content read from disk (re-read just before writing), and keeps every other key and the body as on disk.
  *Acceptance:* an agent changes a card's body while the card is closed. The user then drags that card: the file has the new status AND the agent's new body.

### Packaging and release (PKG)
- **FR-PKG-01 [Must]** Release builds for macOS (Apple Silicon `.dmg`), Windows (x64 installer or portable `.exe`) and Linux (x64 AppImage), built by CI on every version tag and attached to a GitHub release.
- **FR-PKG-02 [Must]** The README covers install, the unsigned-app warning on each OS, connecting a folder, the file format, `AGENTS.md`, and the before and after size and speed numbers against Electron.

## Standard states
- **Undo and redo:** FR-MOVE-03 (toast), FR-MOVE-05 (board actions), FR-MDE-03 (editor text), FR-VIM-02 (`u`, `Ctrl-r`).
- **Empty states:** FR-FIRST-01 (no projects), FR-FIRST-03 (no matches), plus an empty column showing only "+ Add card" (FR-BOARD-02).
- **Loading and progress:** "Loading folders..." on the first scan of a project, as in Electron. Later refreshes are silent and keep the old cards on screen. Covered by FR-PROJ-08 and FR-LIVE-01.
- **Error states and messages:** FR-PROJ-09 (missing folder), FR-FILE-05 (broken frontmatter), FR-EDIT-09 (file deleted), FR-PREV-04 and FR-PREV-05 (bad maths and diagrams). A failed write shows a toast with the OS error and keeps the edit in the editor so it can be retried.
- **Confirm before destructive actions:** FR-BOARD-05 (archive), FR-EDIT-06 (delete). Remove project never deletes files (FR-PROJ-07).
- **Autosave:** FR-EDIT-02.
- **Search and filter:** FR-FILT-01, FR-PAL-01, FR-MDE-03 (find in card).
- **Keyboard shortcuts:** FR-APP-05, FR-MDE-03, FR-MOVE-04, FR-VIM-02.
- **Settings that persist:** FR-APP-03, FR-APP-07.
- **First run and onboarding:** FR-FIRST-01, FR-FIRST-02.
- **Offline behaviour:** always offline. No network features (NFR-03).
- **Permissions per role:** out of scope: single user, no roles.
- **Notifications:** in-app toasts only (move, copy, errors). No OS notifications: out of scope for v1.
- **Import and export:** out of scope: the files are the format. Nothing to import or export.
- **Delete and restore:** FR-EDIT-06 plus undo (FR-MOVE-05). Archive (FR-BOARD-05) is the soft delete.
- **Audit log:** out of scope: git history in the project is the audit log.
- **Multi-language and RTL:** out of scope (Stage 2: English only, no RTL).
- **Accessibility:** NFR-06.
- **Mobile layout:** out of scope: desktop only. Narrow windows (down to 1024 px) hide the sidebar behind a button, same as Electron's narrow layout.
- **Rate limits and quotas:** out of scope: no network services.

## Account and auth
None. No accounts, sign-in or cloud identity. mdk is a single-user desktop app that reads and writes local folders the user picks.

## Data the product handles
- **Project** : a connected folder (id, display name, absolute path, order in the sidebar). Stored in mdk's config file in the OS config folder. Kept until the user removes it.
- **Card** : one `.md` file inside a project. Frontmatter (nine known keys plus any unknown keys) and a markdown body. Lives in the user's folder, owned by the user. mdk holds a parsed copy in memory only.
- **Work folder** : a project's first-level subfolder. Derived from paths, not stored.
- **Image asset** : image files inside a project, read only for the preview.
- **Saved view** : a name plus filter values, per project. Stored in config.
- **Preferences** : theme, view, editor tab, Vim keys, auto-refresh, selected project and work folder, window state. Stored in config.
- **Undo history** : the board actions of the current session, in memory only, cleared on quit.
- **Sample project** : files written once to a folder the user picks (FR-FIRST-02).

## Integrations
- **The file system** : read, write, rename and delete inside connected folders, and watch them for changes. Two-way.
- **OS folder picker and "save to" picker** : native dialogs.
- **System clipboard** : write only (Copy AI prompt, code-block Copy, preview text copy).
- **Default browser** : opens external links. Outbound only.
- **AI agents** : no direct integration. They edit the same files (FR-AGENT-*).
- No network services, APIs or accounts.

## Non-functional requirements
- **NFR-01 Performance:** mdk must beat the Electron app on all four Stage 1 measures, measured side by side on an M1 with 16 GB using Safi's 169-card board: (a) download size, (b) cold start to a rendered board, (c) idle memory after 10 minutes open, (d) time from a card file being saved by another program to the board showing it. Also, with a generated 1,000-card project: dragging and scrolling stay smooth (no frame over 33 ms in a 10-second scroll), and filter typing updates the board within 100 ms.
- **NFR-02 Privacy and security:** no network requests at all. No telemetry, no update checks, no remote images. File access is limited to connected folders (FR-FILE-09).
- **NFR-03 Offline:** 100% of features work with networking off.
- **NFR-04 Scale:** up to 20 connected projects, 5,000 cards in total, card files up to 1 MB, images up to 20 MB. Beyond that, mdk still works but may be slower.
- **NFR-05 Platforms:** macOS 13+ on Apple Silicon. Windows 10 and 11 x64. Linux x64 (Ubuntu 22.04+ and Fedora 40+, X11 and Wayland). All three tested before each release.
- **NFR-06 Accessibility:** every action is reachable by keyboard, with visible focus. Text contrast is at least 4.5:1 and UI contrast at least 3:1, in both themes. The OS reduce-motion setting is respected. Screen readers are out of scope for v1 (Stage 2).
- **NFR-07 Data safety:** no write ever corrupts or truncates a card (atomic writes, FR-FILE-07). No action deletes a file without confirmation or undo. A file mdk cannot parse is never rewritten.
- **NFR-08 Compatibility:** the Electron app and mdk can be used on the same folder at the same time, without either breaking the other's files.
- **NFR-09 Reusability:** the file store, card format, theme and markdown preview are separate modules with no dependency on the board UI, so later GPUI apps can reuse them.
- **NFR-10 Licence:** MIT. Every dependency must be MIT, Apache-2.0, BSD, ISC, Zlib or another licence compatible with MIT. No GPL or AGPL code, so nothing is copied from Zed's editor, Vim or markdown preview, or from Zorite.

## Out of scope for MVP
- **Browser version and demo projects** : the Electron and web app keeps serving browsers. mdk is desktop only.
- **Screen readers** : Stage 2 decision. Possible v1.1.
- **Right-to-left and Arabic UI** : Stage 2 decision.
- **Full Vim** (registers, macros, marks, text objects, ex commands beyond `:w` and `:q`) : the subset covers daily editing. Could come later.
- **Raw HTML rendering in the preview** : no HTML engine in a native renderer. Shown as text (FR-PREV-10).
- **Remote images** : no network (NFR-02).
- **Custom columns or statuses** : the five statuses are part of the agent contract. Changing them breaks `AGENTS.md`.
- **Sync, accounts and a hosted board** : the Electron repo's sync research (Appwrite, MinIO) stays parked. Files sync through git.
- **Running or orchestrating agents** : Vibe Kanban's territory, not mdk's.
- **OS notifications, reminders for due dates** : in-app only.
- **Mobile and tablet** : desktop only.
- **Signed and notarised builds, auto-update** : unsigned GitHub releases at v1. Signing comes when there's an Apple Developer account.
- **Intel Mac builds** : added later if free (Stage 1).

---

### Open questions
None.

### Assumptions confirmed
1. **Vim : RESOLVED.** A small Vim subset in v1 (FR-VIM-02), built in-house because gpui-kit's editor has none and Zed's Vim is GPL.
2. **Maths and Mermaid : RESOLVED.** Both in v1, at parity with Electron (FR-PREV-04, FR-PREV-05).
3. **Edit clash : RESOLVED.** Reload silently when there are no unsaved edits. Ask with "Keep mine" or "Load theirs" when there are (FR-EDIT-08).
4. **First run : RESOLVED.** A welcome state with "Create sample project", which writes a sample folder where the user chooses (FR-FIRST-02).
5. **Which files are cards : RESOLVED.** Every `.md` file, exactly like Electron (FR-FILE-01). Users connect a tasks folder rather than a repo root.
6. **Board undo : RESOLVED.** Must in v1 (FR-MOVE-05), including undo of delete.
7. **Settings dialog : RESOLVED.** Should (FR-APP-07). The sidebar already holds the toggles.
