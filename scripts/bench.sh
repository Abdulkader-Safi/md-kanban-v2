#!/usr/bin/env bash
# The NFR-01 numbers for one app on this machine: size, cold start, idle memory.
# Run it once for mdk and once for the Electron MD Kanban, on the same folders, and
# compare. macOS only.
#
#   scripts/bench.sh dist/mdk.app                    # after: cargo packager --release -c Packager.toml --formats app
#   scripts/bench.sh "/Applications/MD Kanban.app"
#   IDLE_SECONDS=600 scripts/bench.sh dist/mdk.app   # the 10-minute idle reading NFR-01 asks for
#
# Edit-to-screen latency is not measured here: it needs the frame in which a changed card
# appears, which only the app can know. For mdk, the watcher tests bound it at 500 ms.
# A 1,000-card project to test with:  scripts/bench.sh --generate /tmp/mdk-1000
set -euo pipefail

if [ "${1:-}" = "--generate" ]; then
  dir="${2:?usage: bench.sh --generate <folder>}"
  statuses=(backlog todo in-progress review done)
  priorities=(critical high medium low)
  for i in $(seq 1 1000); do
    mkdir -p "$dir/work-$((i % 8))"
    cat > "$dir/work-$((i % 8))/card-$i-2026-10-11.md" <<CARD
---
id: "card-$i-2026-10-11"
status: "${statuses[$((i % 5))]}"
priority: "${priorities[$((i % 4))]}"
assignee: "safi"
dueDate: null
created: "2026-10-11T09:00:00.000Z"
modified: "2026-10-11T09:00:00.000Z"
labels: ["bench", "work-$((i % 8))"]
order: $i
---

# Card $i

Notes for card $i. Some text so the card has an excerpt on the board.

- [ ] one
- [x] two
CARD
  done
  echo "1000 cards written to $dir"
  exit 0
fi

app="${1:?usage: bench.sh <path to .app>}"
idle="${IDLE_SECONDS:-30}"
binary="$app/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")"

echo "app:            $app"
echo "size on disk:   $(du -sh "$app" | cut -f1)"
dmg="$(mktemp -u).dmg"
hdiutil create -quiet -volname bench -srcfolder "$app" -format UDZO "$dmg"
echo "compressed dmg: $(du -h "$dmg" | cut -f1)"
rm -f "$dmg"

# Cold start: from launch until the process has a window on screen.
name="$(basename "$binary")"
start=$(python3 -c 'import time; print(time.time())')
"$binary" >/dev/null 2>&1 &
pid=$!
window=""
for _ in $(seq 1 200); do
  window=$(osascript -e "tell application \"System Events\" to count windows of (first process whose unix id is $pid)" 2>/dev/null || true)
  [ "${window:-0}" -ge 1 ] 2>/dev/null && break
  sleep 0.05
done
end=$(python3 -c 'import time; print(time.time())')
if [ "${window:-0}" -ge 1 ] 2>/dev/null; then
  python3 -c "print(f'cold start:     {($end - $start) * 1000:.0f} ms to first window')"
else
  echo "cold start:     no window seen (is the screen locked, or is System Events not allowed?)"
fi

echo "idle for ${idle}s..."
sleep "$idle"
# Electron runs several processes. Add up every process whose command starts with the app.
rss=$(ps -axo rss=,command= | awk -v app="$app" 'index($0, app) { sum += $1 } END { printf "%.0f", sum / 1024 }')
echo "idle memory:    ${rss} MB after ${idle}s (all of the app's processes)"
kill "$pid" 2>/dev/null || true
pkill -f "$app/Contents" 2>/dev/null || true
echo "$name done"
