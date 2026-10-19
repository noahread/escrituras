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
    sips -Z 900 -s format jpeg -s formatOptions 45 "$OUT/$name.png" --out "$OUT/$name.jpg" >/dev/null
    echo "===SCREENSHOT $name==="
    base64 -i "$OUT/$name.jpg" | fold -w 160
    echo "===END==="
  fi
  echo "--- app output ($name) ---"
  cat "$OUT/$name.log"
}

# Saved verses and notes, written straight into the study database the app
# shares with the terminal app (created by the first launch)
seed_study_data() {
  db="$HOME/Library/Application Support/escrituras/study.db"
  if [ ! -f "$db" ]; then
    echo "No study database at $db; skipping saved verses and notes"
    return
  fi
  now=$(date +%s)
  sqlite3 "$db" <<SQL
INSERT OR IGNORE INTO saved_verses (verse_title, saved_at) VALUES
  ('Ether 12:27', $now), ('Moroni 10:4', $now), ('Alma 32:21', $now);
INSERT OR REPLACE INTO notes (verse_title, body, created_at, updated_at) VALUES
  ('Ether 12:27', 'Weakness is given so that we stay humble; grace makes weak things strong.', $now, $now),
  ('Alma 32:21', 'Faith is hope in things which are not seen, which are true.', $now, $now);
SQL
}

shoot reader -Open "Alma 32:21" -ShowInspector YES
shoot search -Search "faith hope charity"
shoot ask -Open "Moroni 10:4" -ShowInspector YES -InspectorTab ask
seed_study_data
shoot study -Open "Ether 12:27" -ShowInspector YES -InspectorTab study
shoot note -Open "Alma 32:21" -EditNote YES
shoot goto -Open "John 3:16" -GoToReference YES
shoot settings -Open "John 3:16" -OpenSettings YES
shoot dark -Open "Doctrine and Covenants 4:2" -ShowInspector YES -Appearance dark
