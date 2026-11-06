#!/usr/bin/env bash
# Make a DRAFT GitHub release of a version, with the update feed in it. Nothing reads a draft:
# installed apps see the version only once someone publishes it on GitHub.
#
#   scripts/make-release.sh <version> <notes file> [--dry-run]
#
# Before it: scripts/package-mac.sh on a Mac (target/release/bundle/), and the tag v<version>
# pushed, with its Windows and Linux runs finished (their packages are fetched from the runs).
#
# Needs SNOUT_UPDATE_KEY, the file of the release signing key (in the environment, or in
# .signing.env at the repository root, gitignored). Every installed app trusts that key and no
# other: see docs/updates.md. GH_TOKEN names who the release is made as. --dry-run stops before the
# release is made, leaving the files in target/release/publish/<version>/ to look at.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VERSION="${1:-}"
NOTES="${2:-}"
DRY="${3:-}"
if [[ -z "$VERSION" || ! -f "$NOTES" ]]; then
  sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
fi
# shellcheck disable=SC1091
[[ -f "$ROOT/.signing.env" ]] && source "$ROOT/.signing.env"
[[ -f "${SNOUT_UPDATE_KEY:-}" ]] || { echo "SNOUT_UPDATE_KEY does not name the signing key." >&2; exit 1; }
BUILT="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[[ "$BUILT" == "$VERSION" ]] || { echo "Cargo.toml says $BUILT, not $VERSION." >&2; exit 1; }

OUT="$ROOT/target/release/publish/$VERSION"
mkdir -p "$OUT"
shopt -s nullglob
for f in target/release/bundle/SnoutCode-"$VERSION"-*.zip target/release/bundle/SnoutCode-"$VERSION"-*.dmg; do
  cp "$f" "$OUT/"
done

# The tag's Windows and Linux packages, once, unless they are here already.
if [[ "$DRY" != "--dry-run" ]]; then
  for workflow in windows.yml linux.yml; do
    run="$(gh run list --workflow "$workflow" --branch "v$VERSION" --status success --limit 1 --json databaseId --jq '.[0].databaseId')"
    [[ -n "$run" ]] || { echo "No finished $workflow run of v$VERSION." >&2; exit 1; }
    work="$(mktemp -d)"
    gh run download "$run" --pattern 'snout-code-*' --dir "$work"
    find "$work" -type f \( -name '*-setup.exe' -o -name '*.deb' -o -name '*.pkg.tar.zst' \) -exec cp {} "$OUT/" \;
    rm -rf "$work"
  done
fi

cargo run -q -p snout-update --bin snout-update-sign -- feed "$SNOUT_UPDATE_KEY" "$VERSION" "$NOTES" "$OUT"
ls -lh "$OUT"

if [[ "$DRY" == "--dry-run" ]]; then
  echo "Dry run: no release made."
  exit 0
fi
gh release create "v$VERSION" --draft --verify-tag --title "SnoutCode $VERSION" --notes-file "$NOTES" "$OUT"/*
echo "Draft release of v$VERSION made. Installed apps see it once it is published."
