#!/bin/bash
# Launch target/Escrituras.app in a few states and describe each screen in the
# CI log: the window frame and all visible text with positions (via
# ci-describe-screen.swift), plus a small JPEG thumbnail as base64 between
# ===SCREENSHOT name=== and ===END=== markers.
# macOS only; used by the CI workflow after scripts/build-mac-app.sh.

set -u

ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP_BIN="$ROOT/target/Escrituras.app/Contents/MacOS/Escrituras"
OUT="$ROOT/target/screenshots"
mkdir -p "$OUT"

shoot() {
  name="$1"
  shift
  "$APP_BIN" "$@" >"$OUT/$name.log" 2>&1 &
  pid=$!
  sleep 10
  screencapture -x "$OUT/$name.png" || echo "screencapture failed for $name"

  echo "===DESCRIPTION $name==="
  if [ -f "$OUT/$name.png" ]; then
    # Run while the app is still open so its window can be listed
    swift "$ROOT/scripts/ci-describe-screen.swift" "$OUT/$name.png"
  fi
  echo "===END==="

  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null

  if [ -f "$OUT/$name.png" ]; then
    sips -Z 640 -s format jpeg -s formatOptions 45 "$OUT/$name.png" --out "$OUT/$name.jpg" >/dev/null
    echo "===SCREENSHOT $name==="
    base64 -i "$OUT/$name.jpg" | fold -w 160
    echo "===END==="
  fi
  echo "--- app output ($name) ---"
  cat "$OUT/$name.log"
}

shoot reader -Open "Alma 32:21" -ShowInspector YES
shoot search -Search "faith hope charity"
shoot ask -Open "Moroni 10:4" -ShowInspector YES -InspectorTab ask
