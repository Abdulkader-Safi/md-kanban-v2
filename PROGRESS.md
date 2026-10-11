# Progress

Milestone log for mdk. The build spec is `resources/06-build-spec.md`.

## Status

- M0 de-risk: in progress
- M1 to M8: not started

## Decisions and deviations

- The spec files live in `resources/`, where Safi put them. The build spec calls the folder `docs/spec/`. Not moved.
- The Electron source is read from the local clone at `../MD-Kanban` (commit `edebd29`).

## M0 results

Filled in as each spike finishes.

### File spike: passes on macOS, CI not run yet

`spikes/file-spike`, 6 tests, green on macOS (M1) three runs in a row, about 0.9 s total.

- External edit, rename-save (vim and git style) and our own atomic write all reach the watcher inside 500 ms with the 150 ms debounce.
- A 50-file burst arrives in at most 3 batches inside 500 ms.
- A card written into a subfolder created after the watcher started is seen (FR-LIVE-04).
- Atomic write replaces a file another handle has open, and leaves no temp file.
- The temp file is named `.<name>.mdk-tmp-<pid>`, hidden so a scan skips it. It exists only for the length of one write.

Open: the exit criterion asks for all three CI runners. `.github/workflows/ci.yml` is written, but the repo has no remote yet, so Windows and Linux are unverified.

## M0 Electron baseline

Not measured yet.
