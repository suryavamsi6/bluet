#!/bin/sh
# Install Bluet from this unpacked tarball.
#
#   ./install.sh                  installs into ~/.local
#   sudo PREFIX=/usr/local ./install.sh    installs system-wide
#   ./install.sh --uninstall      removes what this script installed under PREFIX
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
app=dev.bluet.manager

bin="$PREFIX/bin/bluet"
desktop="$PREFIX/share/applications/$app.desktop"
icon="$PREFIX/share/icons/hicolor/scalable/apps/$app.svg"
doc="$PREFIX/share/doc/bluet"
licenses="$PREFIX/share/licenses/bluet"

refresh() {
  command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin" "$desktop" "$icon"
  rm -rf "$doc" "$licenses"
  refresh
  echo "Removed Bluet from $PREFIX."
  exit 0
fi

install -Dm755 "$here/bin/bluet" "$bin"
install -Dm644 "$here/share/icons/hicolor/scalable/apps/$app.svg" "$icon"
mkdir -p "$(dirname "$desktop")"
# Point the launcher at the installed binary so it works even when
# $PREFIX/bin is not on the desktop session's PATH.
sed "s|^Exec=bluet|Exec=$bin|" "$here/share/applications/$app.desktop" > "$desktop"
chmod 644 "$desktop"
mkdir -p "$doc" "$licenses/third-party"
cp "$here/share/doc/bluet/README.md" "$doc/"
cp "$here/share/licenses/bluet/LICENSE-MIT" "$here/share/licenses/bluet/LICENSE-APACHE" "$licenses/"
cp "$here/share/licenses/bluet/third-party/"* "$licenses/third-party/"
refresh

echo "Installed Bluet to $bin."
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) echo "Note: $PREFIX/bin is not on your PATH; the app menu launcher still works." ;;
esac
echo "Bluet needs the system BlueZ service, a Vulkan driver, and pactl (PulseAudio/PipeWire utils) for audio controls."
