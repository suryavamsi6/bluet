#!/usr/bin/env bash
# Regenerate packaging/flatpak/cargo-sources.json from Cargo.lock with
# flatpak-cargo-generator, so flatpak-builder can build without network access.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
# Pinned so a generator change cannot silently alter the build.
rev="${FLATPAK_BUILDER_TOOLS_REV:-9d86d336a39b9af38b86f114b6ce5820fe260f91}"
url="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/${rev}/cargo/flatpak-cargo-generator.py"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl -fsSL "$url" -o "$work/flatpak-cargo-generator.py"

if command -v uv >/dev/null 2>&1; then
  uv run --quiet "$work/flatpak-cargo-generator.py" \
    "$root/Cargo.lock" -o "$root/packaging/flatpak/cargo-sources.json"
else
  python3 -m venv "$work/venv"
  "$work/venv/bin/pip" install --quiet aiohttp tomlkit
  "$work/venv/bin/python" "$work/flatpak-cargo-generator.py" \
    "$root/Cargo.lock" -o "$root/packaging/flatpak/cargo-sources.json"
fi
echo "Wrote packaging/flatpak/cargo-sources.json"
