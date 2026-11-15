#!/usr/bin/env python3
"""Compare a Bluet screenshot against the reference capture.

The reference frame in `artifacts/bluet-native.png` was captured at 1440x1000
from the same app on the same fixture, but window placement leaves a small
constant offset in a live capture made through the desktop-control agent.
This reports the raw difference, the difference after compensating that
offset, and a per-region breakdown so a layout regression is visible instead
of being averaged away by the large flat background areas.

Both sides are cropped by the same amount after alignment, so a region name
always refers to the same pixels on both images.
"""

from __future__ import annotations

import argparse
import sys

import numpy as np
from PIL import Image

REGIONS = {
    "titlebar": (0, 44, 0, 1440),
    "sidebar": (44, 1000, 0, 215),
    "main-header": (44, 140, 215, 1096),
    "filters": (140, 200, 215, 1096),
    "device-list": (200, 700, 215, 1096),
    "inspector-top": (44, 300, 1096, 1440),
    "inspector-audio": (300, 560, 1096, 1440),
    "inspector-rest": (560, 980, 1096, 1440),
    "footer": (900, 1000, 215, 1096),
}


def load(path: str) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)


def channel_sum_diff(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    return np.abs(a - b).sum(axis=2)


def best_offset(ref: np.ndarray, live: np.ndarray, span: int = 6) -> tuple[int, int]:
    """Find the (dx, dy) that best overlays live on ref."""
    h, w = ref.shape[:2]
    best = (float("inf"), 0, 0)
    for dy in range(-2, 3):
        for dx in range(-span, span + 1):
            # Drop a leading strip from one side and a trailing strip from the
            # other, so both crops keep the same shape for any sign of dx/dy.
            a = ref[max(0, dy) : h - max(0, -dy), max(0, dx) : w - max(0, -dx)]
            b = live[max(0, -dy) : h - max(0, dy), max(0, -dx) : w - max(0, dx)]
            if a.shape != b.shape or a.size == 0:
                continue
            value = float(channel_sum_diff(a, b).mean())
            if value < best[0]:
                best = (value, dx, dy)
    return int(best[1]), int(best[2])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "live",
        help="screenshot of the running app, framed at the reference window size",
    )
    parser.add_argument(
        "--reference",
        default="artifacts/bluet-native.png",
        help="reference capture (default: artifacts/bluet-native.png)",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=15.0,
        help="per-region mean channel-sum difference allowed (default: 15.0)",
    )
    args = parser.parse_args()

    ref = load(args.reference)
    live = load(args.live)
    if ref.shape != live.shape:
        print(
            f"FAIL size {live.shape[1]}x{live.shape[0]} != reference "
            f"{ref.shape[1]}x{ref.shape[0]}\n"
            "      Frame the window at the reference size before capturing; a "
            "smaller window shifts every row and invalidates the comparison.",
            file=sys.stderr,
        )
        return 1

    dx, dy = best_offset(ref, live)
    h, w = ref.shape[:2]
    aligned_ref = ref[max(0, dy) : h - max(0, -dy), max(0, dx) : w - max(0, -dx)]
    aligned_live = live[max(0, -dy) : h - max(0, dy), max(0, -dx) : w - max(0, dx)]
    aligned = channel_sum_diff(aligned_ref, aligned_live)

    print(f"raw mean {float(channel_sum_diff(ref, live).mean()):.2f}   "
          f"aligned mean {float(aligned.mean()):.2f} at dx={dx} dy={dy}")

    failures = 0
    missing = 0
    for name, (y0, y1, x0, x1) in REGIONS.items():
        # Map reference coordinates into the aligned (cropped) pair, clamping
        # the few columns the alignment offset trims off an edge.
        ay0, ay1 = y0 - max(0, dy), y1 - max(0, dy)
        ax0, ax1 = x0 - max(0, dx), x1 - max(0, dx)
        ax1 = min(ax1, aligned.shape[1])
        ay1 = min(ay1, aligned.shape[0])
        ax0, ay0 = max(0, ax0), max(0, ay0)
        if ax1 - ax0 < 8 or ay1 - ay0 < 8:
            missing += 1
            continue
        value = float(aligned[ay0:ay1, ax0:ax1].mean())
        flag = "  <-- over threshold" if value > args.threshold else ""
        print(f"  {name:18s} {value:6.2f}{flag}")
        if value > args.threshold:
            failures += 1

    checked = len(REGIONS) - missing
    if failures:
        print(f"FAIL {failures} of {checked} region(s) over {args.threshold}")
        return 1
    print(f"PASS all {checked} regions match the reference within the capture offset.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
