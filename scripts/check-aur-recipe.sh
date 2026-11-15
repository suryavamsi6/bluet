#!/usr/bin/env bash
set -euo pipefail

# Confirm that packaging/aur/PKGBUILD really describes the archive its release
# tag serves.
#
# The checksum in an AUR recipe must match the published bytes exactly, and the
# only stable way to know is to fetch them and hash them. This needs no makepkg,
# so it runs on any machine and in any CI image, including one whose steps run
# as root.
#
#   ./scripts/check-aur-recipe.sh                  # check the published tag archive
#   ./scripts/check-aur-recipe.sh --archive FILE   # check a file you already have
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
recipe="$project/packaging/aur/PKGBUILD"

die() { echo "$*" >&2; exit 1; }
field() { sed -n "s/^$1=//p" "$recipe" | head -n 1; }

[[ -f "$recipe" ]] || die "No recipe at $recipe"

cargo_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project/Cargo.toml" | head -n 1)"
recipe_version="$(field pkgver)"
url="$(field url | tr -d "'")"
# The recipe carries one checksum, wrapped in quotes inside parentheses.
recorded="$(field sha256sums | tr -d "'\"() ")"
[[ -n "$recipe_version" ]] || die "$recipe has no pkgver."
[[ -n "$recorded" ]] || die "$recipe has no sha256sums."

if [[ "$cargo_version" != "$recipe_version" ]]; then
  die "Recipe is for ${recipe_version} but Cargo.toml says ${cargo_version}."
fi

if [[ "${1:-}" == "--archive" ]]; then
  archive="${2:?--archive needs a file}"
  [[ -f "$archive" ]] || die "No such archive: $archive"
else
  work="$(mktemp -d "${TMPDIR:-/tmp}/bluet-aurcheck-XXXXXX")"
  archive="$work/source.tar.gz"
  source_url="${url}/archive/v${recipe_version}.tar.gz"
  echo "Fetching ${source_url}"
  status="$(curl --silent --show-error --location --output "$archive" \
    --write-out '%{http_code}' "$source_url")" || status=""
  [[ "$status" == "200" ]] || die "Could not download ${source_url} (HTTP ${status:-none})."
fi

actual="$(sha256sum "$archive" | awk '{print $1}')"
if [[ "$actual" != "$recorded" ]]; then
  echo "Mismatch for $(basename "$archive")." >&2
  echo "  recipe: $recorded" >&2
  echo "  actual: $actual" >&2
  die "Regenerate the recipe with ./scripts/package-aur.sh."
fi

echo "OK: ${recipe_version} recipe matches the archive (sha256 ${actual})."
