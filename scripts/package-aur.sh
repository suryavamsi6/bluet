#!/usr/bin/env bash
set -euo pipefail

# Generate the AUR recipe for the current Cargo.toml version.
#
# The AUR publishes recipes, not binaries, so the source is the tagged archive
# on GitHub rather than a locally built tarball. The checksum therefore only
# exists once the tag has been pushed, which is why CI runs this after release.
#
#   ./scripts/package-aur.sh                 # fetch the tag archive and checksum it
#   ./scripts/package-aur.sh --archive FILE  # checksum an archive you already have
#
# Output lands in dist/aur/ as PKGBUILD and .SRCINFO, ready to commit to the
# AUR package repository.
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project/Cargo.toml" | head -n 1)"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([A-Za-z0-9.+-]*)$ ]]; then
  echo "Cargo.toml needs a valid package version" >&2
  exit 1
fi

archive=""
if [[ "${1:-}" == "--archive" ]]; then
  archive="${2:?--archive needs a file}"
  [[ -f "$archive" ]] || { echo "No such archive: $archive" >&2; exit 1; }
else
  url="https://github.com/suryavamsi6/bluet/archive/v${version}.tar.gz"
  work="$(mktemp -d "${TMPDIR:-/tmp}/bluet-aur-XXXXXX")"
  archive="$work/bluet-${version}.tar.gz"
  echo "Fetching $url"
  if ! curl --fail --silent --show-error --location --output "$archive" "$url"; then
    echo "Could not download the tag archive. Push the v$version tag first," >&2
    echo "or pass --archive FILE to checksum an existing tarball." >&2
    exit 1
  fi
fi

sha256="$(sha256sum "$archive" | awk '{print $1}')"
output="$project/dist/aur"
mkdir -p "$output"
sed -e "s/@VERSION@/${version}/g" -e "s/@SOURCE_SHA256@/${sha256}/g" \
  "$project/packaging/aur/PKGBUILD.in" > "$output/PKGBUILD"
(cd "$output" && makepkg --printsrcinfo > .SRCINFO)

echo "AUR recipe for bluet ${version} (sha256 ${sha256}):"
echo "  $output/PKGBUILD"
echo "  $output/.SRCINFO"

if [[ -n "${work:-}" ]]; then
  echo "Archive kept at $archive"
fi
