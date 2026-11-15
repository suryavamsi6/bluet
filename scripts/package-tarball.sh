#!/usr/bin/env bash
# Package a built bluet binary as a relocatable tarball for any glibc Linux:
#   dist/bluet-<version>-x86_64-linux.tar.gz and its .sha256
#
# Usage: scripts/package-tarball.sh [path/to/bluet]   (default: target/release/bluet)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

binary="${1:-target/release/bluet}"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
arch="$(uname -m)"
name="bluet-${version}-${arch}-linux"
stage="dist/tarball/${name}"

[[ -x "$binary" ]] || { echo "No executable at ${binary}; run cargo build --release first." >&2; exit 1; }

rm -rf "dist/tarball"
install -d "$stage"
install -Dm755 "$binary" "$stage/bin/bluet"
# The release profile keeps symbols for local debugging; a download does not need them.
strip --strip-unneeded "$stage/bin/bluet"
install -Dm644 packaging/linux/dev.bluet.manager.desktop \
  "$stage/share/applications/dev.bluet.manager.desktop"
install -Dm644 packaging/linux/dev.bluet.manager.svg \
  "$stage/share/icons/hicolor/scalable/apps/dev.bluet.manager.svg"
install -Dm644 README.md "$stage/share/doc/bluet/README.md"
install -Dm644 LICENSE-MIT LICENSE-APACHE -t "$stage/share/licenses/bluet/"
install -d "$stage/share/licenses/bluet/third-party"
install -m644 assets/licenses/* "$stage/share/licenses/bluet/third-party/"
install -m755 packaging/tarball/install.sh "$stage/install.sh"
install -m644 packaging/tarball/INSTALL.txt "$stage/INSTALL.txt"

# Stable bytes for the same inputs: sorted entries, fixed owner, commit-time mtimes.
epoch="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || date +%s)}"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${epoch}" \
  -C dist/tarball -cf - "$name" | gzip -9n > "dist/${name}.tar.gz"
(cd dist && sha256sum "${name}.tar.gz" > "${name}.tar.gz.sha256")

echo "dist/${name}.tar.gz"
cat "dist/${name}.tar.gz.sha256"
