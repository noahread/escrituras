#!/bin/bash
set -e

SKILLS_DIR="${HOME}/.claude/skills"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_SOURCE="${SCRIPT_DIR}/../skills"

# List of skills to install
SKILLS=(
  "scriptures-scripture"
  "scriptures-topical"
  "scriptures-cross-ref"
  "scriptures-compare"
  "scriptures-ponder"
  "scriptures-journal"
  "scriptures-memorize"
  "scriptures-daily"
)

# Extract version from SKILL.md file
get_version() {
  grep "^version:" "$1" 2>/dev/null | sed 's/version: *//' | tr -d ' '
}

# Compare semver: returns 0 if $1 > $2, 1 otherwise
version_gt() {
  local v1="${1:-0.0.0}" v2="${2:-0.0.0}"
  [ "$v1" = "$v2" ] && return 1
  local IFS=.
  local i v1_parts=($v1) v2_parts=($v2)
  for ((i=0; i<3; i++)); do
    local n1="${v1_parts[i]:-0}" n2="${v2_parts[i]:-0}"
    [ "$n1" -gt "$n2" ] && return 0
    [ "$n1" -lt "$n2" ] && return 1
  done
  return 1
}

echo "Checking scripture study skills..."
installed=0
updated=0
skipped=0
updates_available=()

for skill in "${SKILLS[@]}"; do
  SOURCE_FILE="${SKILLS_SOURCE}/${skill}/SKILL.md"
  LOCAL_FILE="${SKILLS_DIR}/${skill}/SKILL.md"
  source_version=$(get_version "$SOURCE_FILE")

  if [ -f "$LOCAL_FILE" ]; then
    local_version=$(get_version "$LOCAL_FILE")
    if version_gt "$source_version" "$local_version"; then
      updates_available+=("$skill:$local_version:$source_version")
    else
      echo "  ✓ /${skill} (v${local_version}, up to date)"
      ((skipped++)) || true
    fi
  else
    # New install
    mkdir -p "${SKILLS_DIR}/${skill}"
    cp "$SOURCE_FILE" "$LOCAL_FILE"
    echo "  ✓ /${skill} (v${source_version}, installed)"
    ((installed++)) || true
  fi
done

# Offer to update outdated skills
if [ ${#updates_available[@]} -gt 0 ]; then
  echo ""
  echo "Updates available:"
  for update in "${updates_available[@]}"; do
    IFS=':' read -r skill local_ver source_ver <<< "$update"
    echo "  /${skill}: v${local_ver} → v${source_ver}"
  done
  echo ""
  printf "Would you like to update these skills? [y/N] "
  read REPLY < /dev/tty
  if [ "$REPLY" = "y" ] || [ "$REPLY" = "Y" ]; then
    for update in "${updates_available[@]}"; do
      IFS=':' read -r skill local_ver source_ver <<< "$update"
      SOURCE_FILE="${SKILLS_SOURCE}/${skill}/SKILL.md"
      LOCAL_FILE="${SKILLS_DIR}/${skill}/SKILL.md"
      cp "$SOURCE_FILE" "$LOCAL_FILE"
      echo "  ✓ /${skill} updated to v${source_ver}"
      ((updated++)) || true
    done
  else
    echo "  Skipped updates"
  fi
fi

echo ""
[ $installed -gt 0 ] && echo "✓ Installed $installed new skills"
[ $updated -gt 0 ] && echo "✓ Updated $updated skills"
[ $skipped -gt 0 ] && echo "  ($skipped already up to date)"
echo ""
echo "Available commands:"
for skill in "${SKILLS[@]}"; do
  echo "  /${skill}"
done
echo ""
echo "Make sure the scriptures MCP server is configured in your Claude settings."
