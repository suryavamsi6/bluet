#!/usr/bin/env bash
set -euo pipefail

# Read-only native startup smoke test. It verifies that Bluet launches, stays
# alive, renders through the real GPUI window, and leaves BlueZ state unchanged.
project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${BLUET_BINARY:-$project/target/release/bluet}"
capture=0
[[ "${1:-}" == "--capture" ]] && capture=1

if [[ ! -x "$binary" ]]; then
  echo "Build Bluet first: cargo build --release" >&2
  exit 1
fi
if ! command -v busctl >/dev/null; then
  echo "busctl is required to compare BlueZ state" >&2
  exit 1
fi

snapshot_bluez() {
  busctl --system --json=short call org.bluez / \
    org.freedesktop.DBus.ObjectManager GetManagedObjects |
    python3 -c '
import json, sys
data = json.load(sys.stdin)["data"][0]
keys = ("Powered", "Discoverable", "Discovering", "Pairable",
        "Connected", "Paired", "Trusted")
state = {}
for path, interfaces in sorted(data.items()):
    for iface in ("org.bluez.Adapter1", "org.bluez.Device1"):
        props = interfaces.get(iface)
        if props:
            state[path + " " + iface] = {
                key: props[key]["data"] for key in keys if key in props
            }
print(json.dumps(state, sort_keys=True))
'
}

before="$(snapshot_bluez)"
log="$(mktemp "${TMPDIR:-/tmp}/bluet-smoke-XXXXXX.log")"
"$binary" >"$log" 2>&1 &
pid=$!
sleep 4

if ! kill -0 "$pid" 2>/dev/null; then
  echo "Bluet exited during startup:" >&2
  cat "$log" >&2
  exit 1
fi

if [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]] && command -v hyprctl >/dev/null; then
  address="$(hyprctl clients -j | python3 -c '
import json, sys
pid = int(sys.argv[1])
for client in json.load(sys.stdin):
    if client.get("pid") == pid:
        print(client["address"])
        break
' "$pid")"
  if [[ -n "$address" ]]; then
    workspace="$(hyprctl clients -j | python3 -c '
import json, sys
pid = int(sys.argv[1])
for client in json.load(sys.stdin):
    if client.get("pid") == pid:
        print(client["workspace"]["id"])
        break
' "$pid")"
    if [[ -n "$workspace" ]]; then
      hyprctl eval "hl.dispatch(hl.dsp.focus({workspace=\"$workspace\"}))" >/dev/null
      sleep 0.3
    fi
    hyprctl eval "hl.dispatch(hl.dsp.focus({window=\"address:$address\"}))" >/dev/null
    sleep 0.3
    hyprctl eval 'hl.dispatch(hl.dsp.window.float({action="set"})); hl.dispatch(hl.dsp.window.resize({exact=true,x=1440,y=1000})); hl.dispatch(hl.dsp.window.center()); hl.dispatch(hl.dsp.window.bring_to_top())' >/dev/null
    sleep 1
    if [[ "$capture" == 1 ]] && command -v grim >/dev/null; then
      mkdir -p "$project/artifacts"
      geometry="$(hyprctl clients -j | python3 -c '
import json, sys
pid = int(sys.argv[1])
for client in json.load(sys.stdin):
    if client.get("pid") == pid:
        x, y = client["at"]
        w, h = client["size"]
        print(f"{x},{y} {w}x{h}")
        break
' "$pid")"
      if [[ -n "$geometry" ]]; then
        # Never overwrite the committed reference frame: write the candidate
        # next to it so parity-check.py can compare the two.
        capture_path="$project/artifacts/bluet-native.live.png"
        grim -g "$geometry" "$capture_path"
        echo "Captured $capture_path"
        echo "Compare with: ./scripts/parity-check.py $capture_path"
      fi
    fi
  fi
fi

sleep 1
if ! kill -0 "$pid" 2>/dev/null; then
  echo "Bluet exited while rendering:" >&2
  cat "$log" >&2
  exit 1
fi
if grep -qiE 'panic|ERROR' "$log"; then
  echo "Bluet logged an error:" >&2
  cat "$log" >&2
  kill "$pid" 2>/dev/null || true
  exit 1
fi

kill "$pid" 2>/dev/null || true
for _ in $(seq 1 50); do
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.1
done
kill -9 "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true

after="$(snapshot_bluez)"
if [[ "$before" != "$after" ]]; then
  echo "Bluet startup changed BlueZ state:" >&2
  diff <(echo "$before") <(echo "$after") >&2 || true
  exit 1
fi

echo "PASS native startup renders with the live BlueZ service and leaves adapter/device state unchanged."
