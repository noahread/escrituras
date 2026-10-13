#!/bin/bash
# Launch target/Escrituras.app in a few states and print downscaled screenshots
# into the CI log as base64 (between ===SCREENSHOT name=== and ===END===
# markers), so the app's appearance can be reviewed from the log.
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
  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null

  if [ -f "$OUT/$name.png" ]; then
    sips -Z 1280 -s format jpeg -s formatOptions 60 "$OUT/$name.png" --out "$OUT/$name.jpg" >/dev/null
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
