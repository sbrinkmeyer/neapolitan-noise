#!/usr/bin/env python3
"""Build native/assets/icon.ico from native/assets/icon.png.

Uses PNG-compressed ICO entries (Vista+), which the MSVC resource compiler
accepts. Requires macOS `sips` for resizing, so run this on a Mac; the resulting
.ico is committed to the repo and CI just consumes it.
"""

import pathlib
import struct
import subprocess
import sys
import tempfile

SIZES = [16, 32, 48, 64, 128, 256]

root = pathlib.Path(__file__).resolve().parents[2]
source = root / "native" / "assets" / "icon-1024.png"
target = root / "native" / "assets" / "icon.ico"

if not source.exists():
    sys.exit(f"missing source icon: {source}")

images = []
with tempfile.TemporaryDirectory() as tmp:
    for size in SIZES:
        out = pathlib.Path(tmp) / f"icon-{size}.png"
        subprocess.run(
            ["sips", "-s", "format", "png", "-z", str(size), str(size),
             str(source), "--out", str(out)],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        images.append((size, out.read_bytes()))

header = struct.pack("<HHH", 0, 1, len(images))
offset = len(header) + 16 * len(images)

directory = b""
payload = b""
for size, data in images:
    directory += struct.pack(
        "<BBBBHHII",
        0 if size == 256 else size,  # 0 encodes 256
        0 if size == 256 else size,
        0,  # palette entries
        0,  # reserved
        1,  # color planes
        32,  # bits per pixel
        len(data),
        offset,
    )
    payload += data
    offset += len(data)

target.write_bytes(header + directory + payload)
print(f"wrote {target} ({len(header + directory + payload)} bytes, {len(images)} sizes)")
