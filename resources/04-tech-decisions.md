# Tech decisions

Status: signed off 2026-10-11

For each decision: what was chosen, what else was considered, and why, traced to a requirement (Stage 3) or the mission and users (Stages 1 and 2). Versions checked on crates.io and GitHub on 2026-10-11.

> **Constraints going in:** Rust only, GPUI plus gpui-kit for all UI (Safi's brief). MIT licence, so no GPL or AGPL code (NFR-10). macOS, Windows and Linux at v1 (Stage 1). No network (NFR-02). One developer (Safi) using Claude Code. $0 running cost. The file format must stay byte-compatible with the Electron app (FR-FILE-02, FR-FILE-03, NFR-08).
> **Changes:** none yet.

## Overall architecture
- **Choice:** a Cargo workspace with one app crate and four library crates. The libraries hold everything that is not board UI, so later GPUI apps can reuse them (NFR-09):
  - `mdk-format`: card parsing and writing.
  - `mdk-store`: projects, the file system, the watcher and atomic writes.
  - `mdk-markdown`: the preview plugins and rendering.
  - `mdk-vim`: the Vim layer.
  - `mdk` (app): GPUI views, state and actions.

  A single in-memory `Board` model (one GPUI entity) holds projects and cards, and is the only thing that writes to disk. The watcher feeds file events into it. Views read from it.
- **Considered:**
  - One crate for everything: simpler at first, but NFR-09 asks for reusable pieces, and separate crates keep the format and store testable without GPUI.
  - Keeping the Vue UI in a GPUI webview (gpui-kit has a `webview` crate): defeats the point of the rebuild (Stage 1 mission).
  - Tauri: smaller than Electron but still a webview and still HTML UI. The brief is GPUI.
- **Why:** the store and format crates have no GPUI dependency, so they're tested with plain `cargo test` (FR-FILE-02, FR-FILE-03, FR-FILE-04 test suites). One writer (the Board) is what makes "never overwrite an agent's edit" enforceable (FR-AGENT-03, FR-EDIT-08).

## UI framework
- **Choice:** `gpui-kit = "=0.7.1"` (released 2026-10-05, Apache-2.0, github.com/longbridge/gpui-kit). GPUI comes through it, pinned by the kit to `gpui-pre =0.3.8` (a snapshot of Zed commit `279fe07`). We don't depend on GPUI directly.
- **Considered:**
  - The official `gpui` crate (0.2.2, Oct 2025): stale, and gpui-kit doesn't build against it.
  - A git dependency on `zed-industries/zed`: newest GPUI, but gpui-kit may not compile against it, and every `cargo update` risks breakage.
  - Community forks (`gpui-ce`, `gpui-box`): smaller communities, and gpui-kit doesn't target them.
  - `gpui-component` (the kit's old name, also 0.7.1): same code. `gpui-kit` is the current umbrella crate.
- **Why:** gpui-kit has about 70 components covering almost all of the UI: button, input, select, date-picker, dialog, alert-dialog, sheet (the card editor), popover, tooltip, notification (toasts), command (⌘K), sidebar, tabs, toggle-group (Board/List), checkbox, switch, badge, tag, scrollable, virtual-list, data-table (list view), kbd, menu (native macOS menu plus an in-window menu bar), title-bar, editor, and text-view (markdown). It has about 16.9k stars and releases weekly. Exact pins keep builds reproducible.
- **Landmines:**
  - GPUI is pre-1.0 and breaks its API between snapshots. Upgrade gpui-kit only on purpose, in its own commit, never with a blanket `cargo update`.
  - Docs are thin. Use gpui-kit.com, the kit's `crates/story` gallery and its `examples/` folder as the reference.
  - Zed's own crates (editor, vim, markdown_preview) are GPL or AGPL. Read them, never copy them (NFR-10).

## Board drag and drop
- **Choice:** GPUI's built-in `on_drag`, `drag_over`, `on_drop` and `on_drag_move` on card and column elements, with a custom drag preview (the card face) and a drop-slot element. This follows the kit's "Drag and Drop Reordering" list recipe. Auto-scroll near the edges is driven from `on_drag_move` (FR-MOVE-01).
- **Considered:** no DnD library exists for GPUI. Building pointer tracking by hand duplicates what GPUI already has.
- **Why:** native drag events, 60 fps rendering, and no extra dependency.
- **Landmines:** no open-source GPUI kanban exists to copy from, so this is spiked in M0. A column holding a GPUI `uniform_list` or `list` must expose item bounds so the drop index can be worked out (FR-MOVE-01).

## Markdown editor
- **Choice:** gpui-kit's `Editor` and `EditorState` (rope-based, tree-sitter highlighting, multi-cursor, search, undo and redo, IME tested) with the `markdown` grammar enabled. Our own toolbar calls our own markdown commands (`mdk-markdown::commands`, a port of Electron's `md-commands.ts`) that edit the text and selection through `EditorState`.
- **Considered:**
  - gpui-kit `Textarea`: plain text, no highlighting.
  - Writing an editor from scratch: months of work.
  - Zed's `editor` crate: GPL.
- **Why:** it covers FR-MDE-01, FR-MDE-03 and FR-MDE-04 out of the box, and the toolbar logic is plain text manipulation that can be tested without GPUI (FR-MDE-02).
- **Landmines:** tree-sitter highlighting colours text but may not change font size, so headings may render bold and coloured at the same size (FR-MDE-01 asks for bigger). If so, this is checked in M0 and the requirement amended. Also check the API for setting the selection from code, which the toolbar needs.

## Vim mode
- **Choice:** our own crate `mdk-vim`: a small state machine (mode, pending operator, count, last search) that receives keystrokes before the editor does (GPUI `capture_key_down` on the editor's parent) and turns them into edits and cursor moves on `EditorState`. In insert mode, keys pass through untouched. The motion and operator logic works on a plain text buffer, so it's tested without GPUI (FR-VIM-02).
- **Considered:**
  - Zed's `vim` crate: GPL and tied to Zed's editor.
  - Dropping Vim: rejected in Stage 3.
  - Embedding a terminal Vim or NeoVim through RPC: a large dependency and process management, overkill for the FR-VIM-02 subset.
- **Why:** the subset is small (about 40 commands) and fits in roughly 1,000 to 1,500 lines with tests.
- **Landmines:** the editor's key bindings (Escape keeps the active cursor, Cmd shortcuts) must not fire in normal mode, which depends on GPUI dispatch order. That is spiked in M0.

## Markdown preview
- **Choice:** gpui-kit's `TextView` (markdown-rs `markdown` 1.0.0, GFM: tables, task lists, footnotes, strikethrough, images, text selection) extended with its `MarkdownPlugin` API (block and inline custom nodes) and its `on_link_click` handler. Our plugins live in `mdk-markdown`:
  - **Callouts:** a block plugin on blockquotes starting `[!type]`, with folding (FR-PREV-06).
  - **Code blocks:** a header with the language or title, plus Copy, using the kit's tree-sitter highlighter (FR-PREV-03).
  - **Maths:** inline and block math nodes rendered with `ratex-gpui` 0.4.1 (MIT, built on gpui-pre ^0.3, KaTeX-compatible layout from the RaTeX project), after the dollar-guard check (FR-PREV-04).
  - **Mermaid:** code nodes with language `mermaid` rendered to SVG by `mermaid-rs-renderer` 0.3.1 (MIT, about 23 diagram types), drawn with GPUI's `svg()` or rasterised with `resvg` at the version gpui-kit already uses. Rendering runs in the background and is cached by source hash (FR-PREV-05).
  - **Wikilinks, embeds and `==highlight==`:** inline plugins on text nodes, skipped inside code, maths and Mermaid (FR-PREV-07, FR-PREV-08, FR-PREV-09).
  - **Comments:** `%% %%` are stripped before parsing, using a port of Electron's `stripComments` (FR-PREV-07).
  - **Images:** a plugin on image nodes that loads bytes from the project through `mdk-store` (image types only), and never from the network (FR-PREV-09, NFR-02).
  - **Raw HTML:** shown as text, with `<br>` as a line break (FR-PREV-10).
- **Considered:**
  - Our own renderer from a `comrak` 0.56 or `pulldown-cmark` 0.13 AST to GPUI elements: full control, but it re-does text layout, selection and tables that TextView already has.
  - Zorite's `zorite-markdown` (MIT, built on gpui-component): a good reference, but a separate project with its own goals.
  - Zed's markdown preview: GPL.
- **Why:** TextView already does the hard parts (wrapping, selection, tables, task lists), and its plugin API covers everything Electron's rehype plugins did. Using the kit's own parser means one markdown parser in the app.
- **Landmines:**
  - Plugin coverage of wikilinks inside text nodes, and of callout folding, is unproven. Spiked in M0.
  - mermaid-rs-renderer is young (0.3.x), and some diagram types may differ from mermaid.js. Zed uses a fork (`zed-industries/mermaid-rs-renderer`), so if the crates.io version falls short, pin that fork by commit (it is also MIT; check at build time).
  - ratex-gpui is 0.4.x, from one maintainer. If it breaks on a GPUI bump, render with `ratex-svg` plus `resvg` instead.

## Card file format (parse and write)
- **Choice:** our own `mdk-format` crate.
  - **Reading:** split the frontmatter on `---` lines exactly as Electron's `splitFrontmatter` does. Parse the YAML with `serde-saphyr` 1.3.0 (MIT or Apache-2.0, panic-free, YAML 1.2) into a map. Map the nine known keys with Electron's defaults (FR-FILE-02).
  - **Writing:** a hand-written serializer emits the nine known keys in Electron's order and quoting (FR-FILE-03). Then it appends **the original source lines of every unknown top-level key, copied verbatim** from the file (FR-FILE-04). Unknown keys keep their exact formatting and comments, and an agent's custom field round-trips byte for byte.
  - **Broken YAML:** the file is kept as raw text and marked broken (FR-FILE-05).
- **Considered:**
  - `serde_yaml`: deprecated and archived.
  - `serde_yml`: deprecated, now a shim over `noyalib`.
  - A full serde round trip with `#[serde(flatten)]` extra keys: loses formatting and comments of unknown keys.
  - `yaml-edit` 0.3 (lossless): heavier than needed, because we always rewrite the known keys in a fixed format anyway.
- **Why:** byte compatibility with Electron is a hard requirement (NFR-08). A tiny explicit writer is easier to verify than a generic serializer. Copying unknown keys verbatim is the safest way to never damage an agent's data.

## File store and watcher
- **Choice:** `mdk-store`.
  - **Scanning:** `std::fs` walk with Electron's rules (depth 4, skip hidden, `node_modules`, `dist`, `out`, `.git`) (FR-FILE-01).
  - **Atomic writes:** temp file in the same folder, `fsync`, then rename, which is `MoveFileEx` with replace on Windows. Line endings are preserved (FR-FILE-07, FR-FILE-08).
  - **Path guard:** every path is canonicalised and checked to be inside the project root (FR-FILE-09).
  - **Watching:** `notify` 8.2.0 plus `notify-debouncer-full` 0.7.0 (both MIT or Apache-2.0), recursive, debounced at 150 ms, run on a background thread. Events reach the Board through a channel (FR-LIVE-01, FR-LIVE-04).
  - **Own-write suppression:** the store remembers the content hash of every file it writes, and ignores a watcher event whose new content hash matches (FR-LIVE-02).
  - **Re-read before write:** every save re-reads the file, applies only the changed fields, and writes (FR-AGENT-03).
- **Considered:**
  - Polling every 5 seconds like the browser build: slower and wasteful (Stage 1).
  - Per-folder `fs.watch`-style watchers like Electron: `notify` already wraps FSEvents on macOS, ReadDirectoryChangesW on Windows and inotify on Linux.
- **Why:** native OS events with debouncing hit the 500 ms target with no polling. gpui-kit already depends on `notify` ^8.2, so it's not an extra dependency.
- **Landmines:**
  - inotify on Linux has a per-user watch limit (often 8,192 by default). That is fine for task folders, but if `notify` reports the limit, show an error toast.
  - Editors and git often write by rename, which arrives as remove plus create. The debouncer pairs these.

## App state and preferences
- **Choice:** one `config.json` in the OS config folder found by `directories` 6.0.0 (for example `~/Library/Application Support/mdk/` on macOS, `%APPDATA%\mdk\` on Windows, `~/.config/mdk/` on Linux), written atomically, using `serde` and `serde_json`. It holds the projects (id, name, absolute path, order), prefs, saved views and window state (FR-APP-03, FR-APP-04). Nothing is ever written inside project folders (FR-AGENT-02).
- **Considered:** SQLite: too much for a few KB of settings. TOML: fine too, but JSON is already in the dependency tree.
- **Why:** simple, human-readable, and easy to back up or reset.

## Undo and redo
- **Choice:** an app-level command stack in the Board. Each board action (move, archive, create, delete) records its inverse, plus the full original file bytes for delete and archive. ⌘Z and ⌘⇧Z apply them while the board has focus. Inside the text editor, ⌘Z belongs to the editor's own history (FR-MOVE-05, FR-MDE-03). The 5-second toast's Undo uses the same stack (FR-MOVE-03).
- **Why:** one mechanism for the toast and the shortcut, and delete becomes fully reversible.

## Icons and fonts
- **Choice:** the Tabler icons (MIT) that Electron uses, about 25 SVGs, copied into `assets/icons/` and registered with gpui-kit's icon asset source. The fonts are Geist Sans (SIL OFL 1.1) for UI text, bundled, and the system monospace for code, paths and the editor (Menlo on macOS, Consolas on Windows, DejaVu Sans Mono on Linux).
- **Considered:** gpui-kit's built-in Lucide icons. They look slightly different, and Stage 1 says same UI.
- **Why:** pixel parity with the Electron screenshot (FR-CARD-01). OFL fonts are fine to bundle in an MIT app.

## Theme
- **Choice:** a custom gpui-kit theme (JSON, two modes) that copies Electron's CSS variables: background, foreground, card, muted, secondary, accent, border, primary (teal), destructive, sidebar tokens, the editor `--md-*` syntax colours, and the five column colours. Square chips (radius 0) match Electron's square-chips change. A "System" mode follows the OS appearance (FR-APP-02). Contrast is checked with a script against NFR-06 (4.5:1 text, 3:1 UI).
- **Why:** gpui-kit theming is token-based, so one theme file reproduces the look across every component.

## Packaging and distribution
- **Choice:** `cargo-packager` 0.11.8 (Apache-2.0 or MIT, crabnebula-dev), with one `Packager.toml`:
  - macOS: `.app` and `.dmg`, aarch64.
  - Windows: NSIS installer `.exe`, per-user install, x64.
  - Linux: AppImage, x64.

  Builds are unsigned at v1 (FR-PKG-01).
- **Considered:**
  - Manual scripts per OS (gpui-kit's packaging guide): more control, but three scripts to maintain.
  - `cargo-bundle` 0.12: active, but weaker on Windows and AppImage.
  - MSI: needs WiX, and is enterprise-oriented.
- **Why:** one tool and one config for all three formats.
- **Landmines:**
  - cargo-packager's last release was Nov 2025. If it breaks with the toolchain, fall back to gpui-kit's manual macOS steps plus `cargo-packager` for Windows and Linux only.
  - Unsigned macOS apps need right-click then Open. Windows SmartScreen shows "More info, Run anyway". Both go in the README.

## CI/CD
- **Choice:** GitHub Actions.
  - On every push and PR: `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` on `macos-14` (Apple Silicon), `windows-latest` and `ubuntu-22.04`.
  - On a `v*` tag: build and package on the same three runners and attach the artefacts to a GitHub release (FR-PKG-01).
  - A licence check with `cargo-deny` that fails on GPL or AGPL dependencies (NFR-10).
- **Why:** free for public repos, and it covers all three OSes from day one.

## Testing
- **Choice:**
  - Unit tests in `mdk-format`, `mdk-store`, `mdk-vim` and `mdk-markdown::commands`, with no GPUI.
  - Golden-file tests run every card file in the Electron repo's `tasks/` and `examples/` folders through parse and write (FR-FILE-02, FR-FILE-03).
  - Watcher integration tests write files from a separate thread and assert board updates within 500 ms.
  - GPUI view tests use gpui-kit's headless test context for dialogs, palette and keyboard moves.
  - A scripted performance run measures the four NFR-01 numbers for mdk and Electron.
- **Why:** most of the risk is in the format and the file handling, which are fully testable without a window.

## Distribution of the agent contract
- **Choice:** copy `AGENTS.md` and the Claude skill from the Electron repo into mdk's repo (renaming the skill to `skills/mdk/SKILL.md`), unchanged except for the app name (FR-AGENT-01).

## Not used
- **Database:** none. The files are the database.
- **Auth, hosting, payments, email, analytics, network:** none (NFR-02).
- **Background jobs:** only the in-process watcher thread and the preview's diagram and maths rendering tasks.

## Reused from existing work
- **Electron MD Kanban (MIT, Safi's):**
  - Logic to port to Rust with its behaviour kept: `types.ts`, `markdown.ts` (parse, write, subtasks, ids), `quick-add.ts`, `format.ts` (due labels), `comments.ts`, `md-commands.ts`, and the filter and move logic in `stores/board.ts`.
  - Tests to port: `md-commands.check.ts` and `render.check.ts`.
  - Assets: the theme colours from `style.css`, `build/app-icon.svg`, `AGENTS.md`, `skills/md-kanban/SKILL.md`, and `examples/demo-project/` as the start of the sample project (FR-FIRST-02).
- **gpui-kit examples:** `examples/markdown` (TextView plugins, mention plugin), `crates/story/src/stories/list_story.rs` (drag reordering) and the packaging doc.

## Cost
$0 a month. No hosting, no APIs, no paid licences. GitHub Actions is free for public repos. Optional later: an Apple Developer account at $99 a year for signing and notarisation (out of scope for v1).

## Licensing
- **Project:** MIT.
- **Dependency licences:**
  - gpui-kit and gpui-pre: Apache-2.0.
  - ratex-gpui: MIT.
  - mermaid-rs-renderer: MIT.
  - serde-saphyr, notify, notify-debouncer-full, directories: MIT or Apache-2.0.
  - markdown-rs: MIT.
  - tree-sitter-highlight: MIT.
  - resvg: Apache-2.0 or MIT.
  - Tabler icons: MIT.
  - Geist font: OFL 1.1.

  All are compatible with MIT distribution. Apache-2.0 components need their NOTICE and licence text in the bundle's third-party licences file, generated with `cargo-about`.
- **Check at build time:** that each tree-sitter grammar gpui-kit enables is MIT or Apache, and the licence of the Zed fork of mermaid-rs-renderer if we switch to it. `cargo-deny` enforces both in CI.

## Top technical risks
1. **Card drag and drop across five scrolling columns in GPUI.** No GPUI kanban exists to learn from, and drop-index maths inside virtualised lists plus edge auto-scroll is fiddly. Spike: M0 builds five columns of 200 fake cards each and drags across them on all three OSes.
2. **Vim keys on gpui-kit's Editor.** Key interception order and setting the selection from code are unverified. Spike: M0 proves `hjkl`, `dd`, `i`, `Esc` and visual `d` on an EditorState.
3. **Preview plugins.** Wikilinks inside text, folding callouts, ratex-gpui maths and mermaid-rs-renderer diagrams in TextView. Spike: M0 renders one card containing all of them.
4. **Cross-platform file watching and atomic writes.** Behaviour differs on Windows (rename over an open file) and on Linux (inotify limits). Spike: the M0 watcher test runs on all three CI runners.
5. **GPUI churn.** A gpui-kit upgrade can break the build. Mitigation: exact pins, upgrades in their own branch, and CI on all three OSes.

---

### Open questions
None. All tech choices are confirmed below or follow from Stages 1 to 3.

### Assumptions confirmed
1. **Icons : RESOLVED.** Bundle the roughly 25 Tabler SVGs Electron uses, for pixel parity.
2. **GPUI pin : RESOLVED.** `gpui-kit =0.7.1` with its `gpui-pre =0.3.8` pin. GPUI and the kit are upgraded together, on purpose.
3. **Packaging : RESOLVED.** cargo-packager.
4. **Windows format : RESOLVED.** NSIS installer `.exe`, per-user, no admin needed.
