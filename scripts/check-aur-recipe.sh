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
#
# The recipe is generated into dist/aur/ rather than committed, because it
# embeds a checksum of the archive that contains it; see scripts/package-aur.sh.
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
recipe="${AUR_RECIPE:-$project/dist/aur/PKGBUILD}"

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

# .SRCINFO must describe the same recipe. It is a projection of the PKGBUILD, so
# a stale or malformed one (an empty "key = " line, or an optdepends entry split
# into one word per line) would be published to the AUR as-is and confuse every
# client that reads it.
srcinfo="${AUR_SRCINFO:-$(dirname "$recipe")/.SRCINFO}"
if [[ -f "$srcinfo" ]]; then
  if grep -qE '^[[:space:]]+[a-z]+ =[[:space:]]*$' "$srcinfo"; then
    die "$srcinfo has an empty field value, which no client can parse."
  fi
  srcinfo_version="$(sed -n 's/^\tpkgver = //p' "$srcinfo" | head -n 1)"
  [[ "$srcinfo_version" == "$recipe_version" ]] \
    || die "$(basename "$srcinfo") is for ${srcinfo_version:-nothing} but the recipe is ${recipe_version}."
  # Every optdepends sentence in the recipe must survive as one .SRCINFO line.
  while IFS= read -r entry; do
    grep -qxF "$(printf '\toptdepends = %s' "$entry")" "$srcinfo" \
      || die "$(basename "$srcinfo") does not carry the optdepends entry: $entry"
  done < <(sed -n "s/^optdepends=(//p" "$recipe" | tr -d "'\")" | tr ',' '\n' | sed 's/^ *//;s/ *$//' | sed '/^$/d')
fi

echo "OK: ${recipe_version} recipe matches the archive (sha256 ${actual})."
