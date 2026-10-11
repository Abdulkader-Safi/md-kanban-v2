# mdk

The native MD Kanban. A kanban board for watching what your AI coding agents are doing: every card is a markdown file in your project folder, agents move cards by editing files, and the board updates live.

mdk is a Rust rebuild of [MD Kanban](https://github.com/Abdulkader-Safi/md-kanban) (Electron) on GPUI and gpui-kit. It reads and writes the same card files, so both apps work on the same folders.

## Status

In progress. The build follows `resources/06-build-spec.md`, milestone by milestone, and `PROGRESS.md` has the current state.

- Done: the M0 spikes (file watching, drag and drop, Vim keys, markdown preview), the card format crate and the file store crate.
- Not built yet: the app window. There is no `mdk` binary to run so far.

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

Not available yet. When the app crate lands (milestone M2) this will be:

```sh
cargo run -p mdk
```

## Layout

- `crates/mdk-format`: the card format. Parse and write card files, quick-add, due labels.
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
