# mdk

The native MD Kanban. A kanban board for watching what your AI coding agents are doing: every card is a markdown file in your project folder, agents move cards by editing files, and the board updates live.

mdk is a Rust rebuild of [MD Kanban](https://github.com/Abdulkader-Safi/md-kanban) (Electron) on GPUI and gpui-kit. It reads and writes the same card files, so both apps work on the same folders.

## Status

In progress. The build follows `resources/06-build-spec.md`, milestone by milestone, and `PROGRESS.md` has the current state.

- Works today: connect folders, see their cards in five columns, filter by project and work folder, and watch the board update when another program edits a card file. Drag cards between columns, move them with the "⋯" menu or the keyboard, create cards with quick-add (`Fix login p1 #backend @safi due:friday`), archive done cards, and undo any of it. Click a card to edit it: fields, a markdown editor with a formatting toolbar, and autosave. If an agent changes the file while you have unsaved edits, mdk asks which version to keep.
- Vim keys in the card editor (switch in the sidebar): normal, insert and visual modes, `h j k l w b e 0 ^ $ gg G` with counts, `d c y` with motions, `x dd yy p P u Ctrl-r`, `/` search, `:q`.
- The Preview tab renders the card: tables, task lists, code with a Copy button, maths (`$x^2$`), Mermaid diagrams, GitHub and Obsidian callouts, `==highlights==`, `[[wikilinks]]` that open the card, and images from the project folder. A Subtasks box on top lets you tick checkboxes.
- Filter the board by text, priority, assignee, label and due date, save filter sets as views per project, switch to a sortable list, and use the command palette (Cmd or Ctrl+K) to find a card, create one, move the open one, or run a command.
- Not built yet: the sample project, release builds for download, and the size and speed numbers against Electron.

Keys on the board: arrows move between cards, Enter opens one, Space picks a card up and drops it, Esc cancels, `N` makes a new card, Cmd or Ctrl+Z undoes and Shift+Z redoes, Cmd or Ctrl+K opens the palette, Cmd or Ctrl+comma opens settings.

## What you need

- Rust stable, 1.89 or newer (`rustup update stable`). Built so far with 1.97.1.
- macOS 13+ on Apple Silicon, Windows 10 or 11, or Linux (X11 or Wayland). Development happens on macOS.
- On Linux, GPUI needs its system libraries (see [gpui-kit.com](https://gpui-kit.com)). The window code has not been run on Linux or Windows yet. Only the file watcher is tested there, in CI.
- [bun](https://bun.sh), only to regenerate the card fixtures.

## Run the tests

```sh
cargo test
```

This runs the library crates in `crates/`. No window opens.

## Run the spikes

The spikes are throwaway proofs for the risky parts of the UI. Each opens a window. The first build compiles GPUI and takes a few minutes.

```sh
cargo run -p ui-spike -- dnd       # five columns of 200 cards, drag and drop
cargo run -p ui-spike -- vim       # Vim keys on the markdown editor
cargo run -p ui-spike -- preview   # callouts, maths, Mermaid, wikilinks, images
```

Their tests run headless:

```sh
cargo test -p ui-spike
cargo test -p file-spike
```

## Run the app

```sh
cargo run -p mdk
```

The first build compiles GPUI and takes a few minutes. Click "Connect folder" and pick a folder of markdown cards. To try it on sample cards, clone the [Electron repo](https://github.com/Abdulkader-Safi/md-kanban) and connect its `examples/demo-project` folder.

Settings live in one `config.json` in the OS config folder (`~/Library/Application Support/mdk` on macOS, `~/.config/mdk` on Linux, `%APPDATA%\mdk` on Windows). mdk writes nothing but card files inside your project folders.

Environment variables:

- `MDK_CONFIG_DIR`: use another folder for `config.json`.
- `MDK_NO_WATCH=1`: turn the file watcher off.

## Layout

- `crates/mdk-format`: the card format. Parse and write card files, quick-add, due labels.
- `crates/mdk`: the app. Board state, preferences, theme and views.
- `crates/mdk-markdown`: the formatting toolbar's text commands, and the rendered preview with its plugins.
- `crates/mdk-vim`: a small Vim over any text buffer.
- `crates/mdk-store`: file access for a project folder. Scan, guarded reads, atomic writes, live watching.
- `spikes/`: the M0 spikes.
- `resources/`: the product and build spec.
- `PROGRESS.md`: milestone log, decisions and deviations from the spec.

## Regenerate the card fixtures

The golden tests compare mdk with the Electron app's own parser. To rebuild the expected output from a checkout of the Electron repo:

```sh
cd crates/mdk-format/tests/fixtures
bun generate.ts /path/to/md-kanban
```

## Licence

MIT.
