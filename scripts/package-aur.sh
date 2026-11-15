#!/usr/bin/env bash
set -euo pipefail

# Generate the AUR recipe for the current Cargo.toml version.
#
# The AUR publishes recipes, not binaries, so the source is the tagged archive
# on GitHub rather than a locally built tarball. The checksum therefore only
# exists once the tag has been pushed, so this runs after a release tag.
#
#   ./scripts/package-aur.sh                 # fetch the tag archive and checksum it
#   ./scripts/package-aur.sh --archive FILE  # checksum an archive you already have
#
# The result is written to dist/aur/, which is deliberately not committed. A
# recipe records a checksum of the archive, the archive contains the committed
# tree, and git stamps every archived file with the commit time, so committing
# a recipe changes the very bytes it describes and no committed recipe can stay
# valid. CI therefore generates the recipe from the published tag and pushes it
# straight to the AUR repository, where it is version controlled from then on.
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project/Cargo.toml" | head -n 1)"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([A-Za-z0-9.+-]*)$ ]]; then
  echo "Cargo.toml needs a valid package version" >&2
  exit 1
fi

url="https://github.com/suryavamsi6/bluet/archive/v${version}.tar.gz"
archive=""
if [[ "${1:-}" == "--archive" ]]; then
  archive="${2:?--archive needs a file}"
  [[ -f "$archive" ]] || { echo "No such archive: $archive" >&2; exit 1; }
else
  work="$(mktemp -d "${TMPDIR:-/tmp}/bluet-aur-XXXXXX")"
  archive="$work/bluet-${version}.tar.gz"
  echo "Fetching $url"
  status="$(curl --silent --show-error --location \
    --output "$archive" --write-out '%{http_code}' "$url")" || status=""
  if [[ "$status" != "200" ]]; then
    # Distinguish "the tag is not published yet" from a network failure, so a
    # caller that retries only retries the thing that can actually change.
    if [[ "$status" == "404" ]]; then
      echo "The tag v$version is not published at $url" >&2
      echo "Push the tag first, or pass --archive FILE to checksum an existing tarball." >&2
      exit 3
    fi
    echo "Could not download $url (HTTP ${status:-none})" >&2
    exit 4
  fi
fi

sha256="$(sha256sum "$archive" | awk '{print $1}')"

output="$project/dist/aur"
mkdir -p "$output"
sed -e "s/@VERSION@/${version}/g" -e "s/@SOURCE_SHA256@/${sha256}/g" \
  "$project/packaging/aur/PKGBUILD.in" > "$output/PKGBUILD"

# .SRCINFO is a mechanical projection of the recipe's fields. The recipe is our
# own bash file, so reading its arrays is exact, and this avoids needing
# makepkg, which refuses to run as root and exists only on Arch.
# shellcheck disable=SC1090,SC1091
(
  set -u
  source "$output/PKGBUILD"
  echo "pkgbase = ${pkgname}"
  echo "	pkgdesc = ${pkgdesc}"
  echo "	pkgver = ${pkgver}"
  echo "	pkgrel = ${pkgrel}"
  echo "	url = ${url}"
  printf '	arch = %s\n' "${arch[@]}"
  echo "	license = ${license}"
  printf '	makedepends = %s\n' "${makedepends[@]}"
  printf '	depends = %s\n' "${depends[@]}"
  printf '	optdepends = %s\n' "${optdepends[@]}"
  printf '	options = %s\n' "${options[@]}"
  echo "	source = ${pkgname}-${pkgver}.tar.gz::${url}/archive/v${pkgver}.tar.gz"
  echo "	sha256sums = ${sha256}"
  echo ""
  echo "pkgname = ${pkgname}"
) > "$output/.SRCINFO"

echo "AUR recipe for bluet ${version} (sha256 ${sha256}):"
echo "  $output/PKGBUILD"
echo "  $output/.SRCINFO"

if [[ -n "${work:-}" ]]; then
  echo "Archive kept at $archive"
fi
