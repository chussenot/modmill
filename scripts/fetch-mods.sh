#!/usr/bin/env bash
# scripts/fetch-mods.sh — pull real-world public-domain ProTracker mods.
#
# Lesson from the first version encoded here: HTML scraping cannot tell
# you a module's FORMAT (the PD listing mixes .xm/.it/.s3m, and link
# markup guesses failed twice). The only reliable format signal is the
# file itself — 'M.K.' at offset 1080 — so this version probes bytes,
# not markup: discover ids -> download each -> keep the first N that
# ARE 4-channel M.K. modules -> attribute from the module's own header
# plus its verifiable page URL. Nothing is guessed.
#
# Modes:
#   scripts/fetch-mods.sh discover [N]   auto-harvest N modules (default 4)
#   scripts/fetch-mods.sh               use the hand-curated MODS list below
#
# Real mods are TEST DATA, not fixtures: they land in testdata/mods/
# (gitignored); the committed conformance suite stays gen_fixtures.py.
set -euo pipefail

DEST="testdata/mods"
API="https://api.modarchive.org/downloads.php?moduleid="
PAGE="https://modarchive.org/index.php?request=view_by_license&query=publicdomain&page="
MODPAGE="https://modarchive.org/index.php?request=view_by_moduleid&query="
UA="Mozilla/5.0 (modmill fetch-mods; test-data harvester)"
PROBE_CAP=60          # politeness ceiling on API downloads per run
SLEEP=1               # seconds between probes

# Hand-curated mode: "moduleid|filename|Title|Author|license-url"
MODS=()

mkdir -p "$DEST"
grep -q '^testdata/' .gitignore 2>/dev/null || echo 'testdata/' >> .gitignore
ATTR="$DEST/ATTRIBUTION.md"
[ -f "$ATTR" ] || printf '# Real-world test modules\n\nSource: The Mod Archive, public-domain license section.\nFormat verified from file bytes (M.K. @1080); titles read from the\nmodule header itself; authorship per the linked module page.\n\n' > "$ATTR"

is_mk() { [ "$(dd if="$1" bs=1 skip=1080 count=4 2>/dev/null)" = "M.K." ] && [ "$(wc -c <"$1")" -ge 2108 ]; }

header_title() {  # bytes 0..19 of a MOD are the song title
  dd if="$1" bs=1 count=20 2>/dev/null | tr -cd '[:print:]' | sed 's/[[:space:]]*$//'
}

keep() { # $1=tmpfile $2=id $3=outname $4=title $5=author-note
  mv "$1" "$DEST/$3"
  local sha; sha="$(sha256sum "$DEST/$3" | cut -d' ' -f1)"
  printf -- '- **%s** — "%s" (%s). %s%s. sha256 %s\n' \
    "$3" "${4:-untitled}" "$5" "$MODPAGE" "$2" "$sha" >> "$ATTR"
  echo "KEEP   $3 — \"${4:-untitled}\""
}

smoke() { # a real-world parse failure is a FINDING, keep the file
  command -v cargo >/dev/null && [ -f Cargo.toml ] || return 0
  cargo run -q --release -- parse "$1" --json >/dev/null 2>&1 \
    || echo "NOTE   $(basename "$1") kept but 'modmill parse' failed — file a bead" >&2
}

if [ "${1:-}" = "discover" ]; then
  WANT="${2:-4}"
  echo "discover: harvesting ids from the public-domain listing..."
  ids="$(for p in 1 2 3 4 5 6 7; do
           curl -sL -A "$UA" "${PAGE}${p}"; sleep "$SLEEP"
         done | grep -oE '(query|moduleid)=[0-9]{4,}' | grep -oE '[0-9]{4,}' | sort -un)"
  [ -n "$ids" ] || { echo "discover: page yielded no ids — site layout changed or blocked; fall back to the manual MODS list" >&2; exit 3; }
  total=$(echo "$ids" | wc -l); echo "discover: $total candidate ids; probing (cap $PROBE_CAP) until $WANT are M.K...."
  kept=0 probed=0
  while IFS= read -r id; do
    [ "$kept" -ge "$WANT" ] && break
    [ "$probed" -ge "$PROBE_CAP" ] && { echo "probe cap reached ($PROBE_CAP)"; break; }
    out="$DEST/modarchive-$id.mod"
    [ -f "$out" ] && { echo "have   $(basename "$out")"; kept=$((kept+1)); continue; }
    probed=$((probed+1)); sleep "$SLEEP"
    tmp="$(mktemp)"
    if ! curl -fsSL -A "$UA" "$API$id" -o "$tmp"; then rm -f "$tmp"; continue; fi
    if is_mk "$tmp"; then
      keep "$tmp" "$id" "modarchive-$id.mod" "$(header_title "$tmp")" "author: see page"
      smoke "$out"; kept=$((kept+1))
    else
      rm -f "$tmp"
    fi
  done <<< "$ids"
  echo "done: $kept kept after $probed probes — attribution in $ATTR"
  [ "$kept" -gt 0 ] || { echo "zero M.K. modules in the probed window — widen PROBE_CAP or curate MODS by hand" >&2; exit 4; }
  exit 0
fi

# ---- manual curated mode ----
[ ${#MODS[@]} -gt 0 ] || { echo "MODS list empty — run 'scripts/fetch-mods.sh discover' or fill the list" >&2; exit 2; }
kept=0 skipped=0
for entry in "${MODS[@]}"; do
  IFS='|' read -r id file title author url <<<"$entry"
  out="$DEST/$file"
  [ -f "$out" ] && { echo "have   $file"; kept=$((kept+1)); continue; }
  tmp="$(mktemp)"; echo "fetch  $file (moduleid $id)"
  curl -fsSL -A "$UA" "$API$id" -o "$tmp"
  if is_mk "$tmp"; then
    mv "$tmp" "$out"
    printf -- '- **%s** — "%s" by %s. %s. sha256 %s\n' "$file" "$title" "$author" "$url" \
      "$(sha256sum "$out" | cut -d' ' -f1)" >> "$ATTR"
    smoke "$out"; kept=$((kept+1))
  else
    echo "skip   $file — not a 4ch M.K. module" >&2; rm -f "$tmp"; skipped=$((skipped+1))
  fi
done
echo "done: $kept kept, $skipped skipped — attribution in $ATTR"
