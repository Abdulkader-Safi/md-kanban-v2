# mdk, native MD Kanban

Status: signed off 2026-10-11

## Mission
Prove that a small, owned, native desktop app can replace a web stack in a box, and build the GPUI foundation Safi reuses for his next native tools (Rummage, the image edit tool, and whatever comes after).

MD Kanban already works: the Electron app (github.com/Abdulkader-Safi/md-kanban, last commit 10 Sep 2026) is in daily use on two real projects with 169 cards. This rebuild does not change what the product does. It changes what it is made of: one Rust binary on GPUI and gpui-kit instead of Vue, Vite and a bundled Chromium.

Four reasons, all from Safi, in priority order:
1. **Smaller and faster.** A board you leave open all day should cost almost nothing to run.
2. **Base for more apps.** The file layer, theme, editor and markdown view become reusable crates for later GPUI apps.
3. **Portfolio showcase.** An open-source native app is a stronger proof of skill for clients and engineers than another web app.
4. **Learn GPUI and Rust** on a real product with a known spec, and turn the build into content.

Business model: none. Free, open source, MIT. Success is measured in performance, reuse and reach, not revenue.

## One-liner
A native kanban board for watching what your AI coding agents are doing: every card is a plain markdown file in your project folder, so agents move cards by editing files while you watch the board update live, in an app that starts instantly and uses a fraction of Electron's memory.

## The problem
People running AI coding agents (Claude Code, Codex, Cursor agents) lose track of what the agent has done, what it is doing and what is left. The agent's progress lives in a chat log that scrolls away.

The fix MD Kanban already proved: keep the task list as files the agent can already edit. No account, no API, no connector. The agent has permission to edit the folder, so it can move a card by changing one line of YAML.

The Electron build carries the cost of being a web app in a box:
- A bundled Chromium plus Node for what is a list of text files. An Electron 44 app on macOS ships a framework of well over 100 MB before any app code. (To measure: the exact Electron baseline goes in M0, see open question 2.)
- A five-second polling loop in the browser build, and a Node `fs.watch` tree in the desktop build, to notice agent edits.
- Two runtimes (browser renderer plus Electron main) talking over IPC for every file read and write.

Existing tools and their gap:
- **Backlog.md** (github.com/MrLesk/Backlog.md, MIT): markdown task files plus a terminal and web kanban, built for AI agents. Closest in idea. It is a CLI with a web view, and it owns its own folder layout (`backlog/`), not your project's folders.
- **Vibe Kanban** (github.com/BloopAI/vibe-kanban): a board that orchestrates coding agents, runs them in worktrees and reviews their diffs. It drives the agents; it is not a passive view of plain files.
- **Obsidian Kanban plugin**: a whole board lives in one markdown file, so an agent editing one card rewrites the board file. Needs Obsidian.
- **Linear, GitHub Projects, Jira**: cloud boards. Agents need an API key, a connector and network access to move a card.

The gap: a native, fast, offline board where one card is one file in your own folder layout, with a rich markdown editor and preview, that never asks the agent to do anything but edit a file.

## The value
**The "after" picture:** Safi starts Claude Code on a feature and opens MD Kanban next to it. The app is up in under a second. As the agent works, cards slide from To Do to In Progress to Review on their own, because the agent edited `status:` in the card files. Safi opens a card, reads the agent's notes rendered with code highlighting and a Mermaid diagram, ticks a subtask, and drags the card to Done. The board has been open for eight hours and the machine has not noticed.

Versus the Electron MD Kanban, the user gets:
- **A small, fast app** : instant start, low idle memory, native scrolling with 500+ cards. Running it all day is free.
- **Live updates without polling** : a native file watcher reacts to agent edits within half a second.
- **Unknown frontmatter keys preserved** : the Electron build drops any key it does not know when it saves a card. The rebuild keeps them, so agents can add their own fields safely.
- **One native app on Mac, Windows and Linux** from one Rust codebase.

Versus Backlog.md and Vibe Kanban, the user gets:
- **Your folders, your layout** : any folder is a project, any subfolder is a work group. No tool-owned directory.
- **A passive view** : the app never runs agents. It reads and writes files. Any agent works with it.

What it deliberately is not:
- Not an agent runner or orchestrator. Vibe Kanban does that.
- Not a sync service or team board. Files sync through git, Dropbox or whatever the user already uses.
- Not a browser app. The Electron and web version (MD Kanban) stays as it is for anyone who wants that.
- Not a new feature set. v1 matches the Electron app's features and look. New features come after parity.

## Product category
A local-first developer productivity tool: a file-backed kanban for AI-assisted development. It competes with Backlog.md on "markdown tasks for agents", with Obsidian Kanban on "boards from markdown", and with Electron MD Kanban on footprint. It is the only one that is native, file-per-card in your own folders, and has a full markdown editor and preview.

## Business model and licence
Free and open source under MIT, same as the Electron app. Distributed as GitHub release downloads. Unsigned builds at first (macOS shows a Gatekeeper warning; the README explains right-click, Open).

## Platforms
macOS, Windows and Linux at v1, all three tested and packaged:
- macOS: `.app` in a `.dmg`, Apple Silicon first, Intel if the build is free.
- Windows: `.msi` or a portable `.exe`.
- Linux: AppImage, plus a `.deb` if cheap.

Development happens on Safi's Mac. Windows and Linux are tested in M0 and before each release.

## What success looks like
**A year in:** the native app is the version Safi uses every day. The Electron repo's README points to it as the recommended download. The GPUI pieces (file store, theme, markdown view) are reused in at least one more of Safi's apps. The build is documented in a blog post and a short video series, and the repo is the project he shows when someone asks "can you build native desktop apps?".

**Measurable at v1.** The gate is "clearly smaller and faster than Electron", measured side by side on the same machine (an M1 with 16 GB) and the same board (Safi's two real projects, 169 cards). The Electron baseline is measured in M0. mdk must beat Electron on all four:
- Download size (compressed `.dmg`).
- Cold start to a rendered board.
- Idle memory with the board open for 10 minutes.
- Time from an agent saving a card file to the board showing the change.

Also required: dragging a card and scrolling a 200-card column stay smooth, with no visible stutter. The before and after numbers go in the README.
- Every feature in the Electron app at commit `edebd29` has a native equivalent or is listed as dropped, with a reason.

## UI and product references
- **The Electron app is the UI reference.** Screenshot: `references/electron-board-screenshot.png`. Source: github.com/Abdulkader-Safi/md-kanban at commit `edebd29` (10 Sep 2026). Keep:
  - Header: logo and "MD Kanban", a "local markdown board, N cards, N projects" line, a Search button showing the ⌘K shortcut, a theme toggle, and a teal "New card N" button.
  - Left sidebar (240 px): teal "Connect folder" button, a PROJECTS list with open-card counts, a WORK FOLDERS list in monospace, and at the bottom Rescan all, "Auto-refresh folders", "Vim keys in editor" and "checked <time>".
  - Filter bar: search input, four priority toggle chips, assignee input, labels select, due select, reset, a filtered/all badge, "save view", and saved-view chips. A Board/List segmented control on the right.
  - Five fixed columns (Backlog grey, To Do blue, In Progress amber, Review violet, Done green), 288 px wide, each with a dot, name, count, "+", and "archive" on Done.
  - Cards: title, priority badge, "⋯" move menu, two-line excerpt, chips for @assignee, due ("3d overdue · 2026-09-10" in red or amber), work folder in monospace and subtasks ("☑ 2/5"), up to three #labels plus "+N more", and the file path in small monospace.
  - Dark theme by default with a teal accent, plus a light theme. Square chips.
  - The card editor is a right-hand side sheet: path, editable title, a four-field grid, Markdown/Preview tabs with an Expand toggle, a formatting toolbar, "Copy AI prompt", Delete, and a collapsible raw-file view.
- **gpui-kit component gallery** (gpui-kit.com, `crates/story` in github.com/longbridge/gpui-kit): the building blocks the UI is made from.
- **Zorite** (github.com/packetThrower/zorite): a GPUI markdown app using gpui-kit, ratex-gpui and Mermaid. Reference for the markdown preview. It is GPLv3, so read it and don't copy it.

---

### Open questions
None.

### Assumptions confirmed
1. **Why rebuild : RESOLVED.** All four reasons: smaller and faster, base for more apps, portfolio showcase, learning GPUI and Rust. Size and speed lead the success criteria.
2. **Platforms : RESOLVED.** Mac, Windows and Linux at v1, all three tested and packaged.
3. **Repo : RESOLVED.** New repo. The Electron and browser app stays as it is and keeps working on the same files.
4. **Licence and distribution : RESOLVED.** MIT, GitHub release downloads, unsigned at first.
5. **Scope : RESOLVED (from Safi's brief).** Same UI and same features as the Electron app, built fully in Rust with GPUI and gpui-kit.
6. **Name : RESOLVED.** The new repo and the app are both called **mdk** (window title "mdk"). The README describes it as the native MD Kanban.
7. **Performance targets : RESOLVED (looser).** No fixed numbers. mdk must beat the Electron app on download size, cold start, idle memory and edit-to-screen time, measured side by side in M0 and again at release.
8. **Mac architectures : RESOLVED.** Apple Silicon only at v1. Intel is added later if it builds with no extra work.
