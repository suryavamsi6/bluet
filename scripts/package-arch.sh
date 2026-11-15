#!/usr/bin/env bash
set -euo pipefail

# Make a checksum-pinned recipe from the current checkout. This deliberately
# does not depend on a public repository, tag, release or GitHub credentials.
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project/Cargo.toml" | head -n 1)"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([A-Za-z0-9.+-]*)$ ]]; then
  echo "Cargo.toml needs a valid package version" >&2
  exit 1
fi
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo "The Bluet Linux package currently supports x86_64 only." >&2
  exit 1
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/bluet-arch-XXXXXX")"
archive_name="bluet-${version}.tar.gz"
archive="$work/$archive_name"
tar \
  --create \
  --gzip \
  --file "$archive" \
  --directory "$project" \
  --sort=name \
  --mtime=@0 \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  --exclude=./.git \
  --exclude=./target \
  --exclude=./artifacts \
  --exclude=./dist \
  --exclude=./result \
  --exclude=./result-* \
  --exclude=./packaging/arch/src \
  --exclude=./packaging/arch/pkg \
  --transform "s,^\\.,bluet-${version}," \
  .

sha256="$(sha256sum "$archive" | awk '{print $1}')"
sed -e "s/@VERSION@/${version}/g" -e "s/@SOURCE_SHA256@/${sha256}/g" \
  "$project/packaging/arch/PKGBUILD.in" > "$work/PKGBUILD"

output="$project/dist/arch"
mkdir -p "$output"
cp "$archive" "$output/$archive_name"
cp "$work/PKGBUILD" "$output/PKGBUILD"
(cd "$work" && makepkg --printsrcinfo > "$output/.SRCINFO")

echo "Source archive and checksum-pinned recipe: $output"
echo "Build directory: $work"

if [[ "${1:-}" != "--prepare-only" ]]; then
  (cd "$work" && makepkg --cleanbuild --force --noconfirm)
  for package in "$work"/*.pkg.tar.zst "$work"/*.pkg.tar.xz; do
    [[ -e "$package" ]] || continue
    cp "$package" "$output/"
    echo "Built $output/$(basename "$package")"
  done
fi
