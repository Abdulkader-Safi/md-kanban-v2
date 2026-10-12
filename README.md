# mdk

The native MD Kanban. A kanban board for watching what your AI coding agents are doing: every card is a markdown file in your project folder, agents move cards by editing files, and the board updates live.

mdk is a Rust rebuild of [MD Kanban](https://github.com/Abdulkader-Safi/md-kanban) (Electron) on GPUI and gpui-kit. It reads and writes the same card files, so both apps work on the same folders.

## Status

Feature-complete against the build spec in `resources/06-build-spec.md`, and tested without a window on macOS, Windows and Linux in CI. The main screens have been looked at in both themes. Still open: checking it by hand next to the Electron app, and publishing a release, so for now you build it from source. `PROGRESS.md` lists every open check.

## Run it

You need Rust stable, 1.89 or newer (`rustup update stable`).

```sh
git clone https://github.com/Abdulkader-Safi/md-kanban-v2 mdk
cd mdk
cargo run --release -p mdk
```

The first build compiles GPUI and takes a few minutes. On Linux, install GPUI's system libraries first. On Ubuntu:

```sh
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev \
  libwayland-dev libfontconfig1-dev libfreetype6-dev
```

Development happens on macOS. On Windows and Linux the tests pass in CI, but the window has not been used by hand yet.

## First steps

1. Click "Create sample project" and pick where to put it. mdk writes a `mdk-sample` folder with seven cards that show every feature.
2. Or click "Connect folder" and pick a folder of your own. Every `.md` file in it is a card. Pick a tasks folder, not a repo root, or the README becomes a card too.
3. Tell your agent: "read AGENTS.md and work through the To Do cards". Watch a card move without touching mdk.

## What it does

- Five columns: Backlog, To Do, In Progress, Review, Done. Drag cards between them, use the "⋯" menu, or use the keyboard.
- Live updates. A native file watcher shows an agent's edit within half a second. No polling.
- New cards in one line: `Fix login p1 #backend @safi due:friday`.
- A card editor with a formatting toolbar, autosave, and optional Vim keys. If an agent changes a file while you have unsaved edits, mdk asks which version to keep.
- A preview with tables, task lists, code, maths (`$x^2$`), Mermaid diagrams, callouts, `==highlights==`, `[[wikilinks]]` and images from the project folder.
- Filters by text, priority, assignee, label and due date, saved as views per project. A sortable list view.
- A command palette, undo and redo for moves, creates, deletes and archives, and "Copy AI prompt" to hand a card to an agent.
- No account, no network requests, no telemetry. mdk writes nothing in your folders except card files.

## Keys

`Cmd` on macOS is `Ctrl` on Windows and Linux.

| Key | What it does |
| --- | --- |
| `N` | New card |
| `Cmd+K` | Command palette |
| Arrows, `Enter` | Move between cards, open one |
| `Space`, arrows, `Space` | Pick a card up, move it, drop it. `Esc` puts it back |
| `Cmd+Z`, `Cmd+Shift+Z` | Undo and redo board actions |
| `Cmd+B`, `Cmd+I`, `Cmd+E` | Bold, italic, inline code in the editor |
| `Cmd+,` | Settings |
| `Esc` | Close the top dialog, sheet or palette |

## The file format

One card is one markdown file with YAML frontmatter:

```markdown
---
id: "login-form-2026-09-06"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: "2026-09-10"
created: "2026-09-06T10:30:00.000Z"
modified: "2026-09-06T14:20:00.000Z"
labels: ["feature", "auth"]
order: 1
---

# Rebuild login form

Task description, checkboxes, notes.
```

- `status` is `backlog`, `todo`, `in-progress`, `review` or `done`. Moving a card changes this one field.
- `priority` is `critical`, `high`, `medium` or `low`.
- The title is the body's first `# ` heading.
- Each first-level subfolder of a project is a work folder. `archive/` is hidden until you select it.
- Frontmatter keys mdk does not know are kept as written when it saves a card, so agents can add their own.
- A card with broken YAML still shows, with a warning. mdk will not move or rewrite it until you fix the YAML in the editor.

mdk writes cards byte for byte the way the Electron app does. That is tested against 46 fixture cards run through Electron's own parser and writer.

## Working with agents

`AGENTS.md` is the contract: the format, the allowed operations, and a recipe for a status check. Copy it into your project folder (the sample project includes it), or point your agent at this repo's copy. For Claude, `skills/mdk/SKILL.md` is the same contract as a skill.

mdk never runs an agent. An agent needs file access to the folder and nothing else.

## Size and speed

Measured on an M1 MacBook, 12 October 2026: mdk 0.1.0 against the Electron app's existing build, with `scripts/bench.sh`.

| | mdk | Electron MD Kanban |
| --- | --- | --- |
| Download (`.dmg`) | 11 MB | 196 MB |
| App on disk | 28 MB | 603 MB |
| Memory after 10 minutes idle, all processes | 67 MB | 250 MB |
| Launch to first window, warm (median of 5) | 515 ms | 521 ms |
| Launch to first window, first run after install | 1.5 s | 3.9 s |

The memory and launch rows were taken with no project connected in either app, because the Electron build on this machine has no saved projects. They are not the 169-card comparison the spec asks for.

Launch time is a tie on this measure. It stops at the first window, and Electron opens its window before its page has loaded, so it does not show which app draws the board first.

mdk only, on a generated 1,000-card project in a release build: a full scan takes 31 ms and one filter keystroke 0.3 ms. The time from an agent saving a file to the board showing it is bounded at 500 ms by the watcher tests. It is not measured for Electron.

## Settings and logs

Everything mdk remembers is in one `config.json` in the OS config folder: `~/Library/Application Support/mdk` on macOS, `~/.config/mdk` on Linux, `%APPDATA%\mdk` on Windows. `mdk.log` sits next to it. The log records what mdk did, never what is in a card.

- `MDK_CONFIG_DIR`: use another folder.
- `MDK_LOG`: log level, `info` by default. `debug` says more.
- `MDK_NO_WATCH=1`: turn the file watcher off.
- `MDK_SHOW`: open screens at launch, for screenshots and manual checks. For example `MDK_SHOW="card=writing/preview-showcase-2026-10-11.md;preview"`. Words: `card=<path>`, `preview`, `markdown`, `list`, `board`, `palette`, `new`, `settings`, `light`, `dark`, `vim`, `project=<name>`.

## Build a release

```sh
cargo install cargo-packager --version 0.11.8 --locked
cargo install cargo-about --locked --features cli
cargo about generate -m crates/mdk/Cargo.toml about.hbs -o THIRD_PARTY_LICENSES.html
cargo packager --release -c Packager.toml
```

This makes a `.app` and `.dmg` on macOS, an NSIS installer on Windows and an AppImage on Linux, in `dist/`. Pushing a `v*` tag runs the same steps on all three in GitHub Actions and attaches the results to a GitHub release.

Builds are unsigned. When you open one:

- macOS says the app is from an unidentified developer. Right-click the app, choose Open, then Open again. You only do this once.
- Windows SmartScreen says it protected your PC. Click "More info", then "Run anyway".
- Linux: `chmod +x mdk_*.AppImage`, then run it.

## Tests

```sh
cargo test                # every crate, 140-odd tests, no window opens
cargo clippy --all-targets -- -D warnings
```

The golden tests compare mdk with the Electron app's own parser. To rebuild the expected output from a checkout of the Electron repo (needs [bun](https://bun.sh)):

```sh
cd crates/mdk-format/tests/fixtures
bun generate.ts /path/to/md-kanban
```

## Layout

- `crates/mdk-format`: the card format. Parse and write card files, quick-add, due labels.
- `crates/mdk-store`: file access for a project folder. Scan, guarded reads, atomic writes, live watching.
- `crates/mdk-markdown`: the formatting toolbar's text commands, and the rendered preview.
- `crates/mdk-vim`: a small Vim over any text buffer.
- `crates/mdk`: the app. Board state, preferences, theme and views.
- `spikes/`: the early proofs for drag and drop, Vim keys and the preview. Kept for reference.
- `resources/`: the product and build spec.
- `PROGRESS.md`: milestone log, decisions, and what is still unchecked.

The four library crates do not depend on the app, so other GPUI apps can use them.

## Licence

MIT. Third-party licences are generated into `THIRD_PARTY_LICENSES.html` and shipped with release builds.
