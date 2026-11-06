#!/usr/bin/env python3
"""Writes themes.json, the window's color themes, from the themes of Rex.

    python3 crates/tokprof-desk/themes/import.py \
        "/Applications/Rex Beta.app/Contents/Resources/SuperUI_SuperDesignTokens.bundle/Contents/Resources/themes.json" \
        "Rex Beta 0.1.0 (1829)"

Rex keeps each theme's semantic roles as Display P3 components and draws them so. The window
draws in sRGB, so each role is converted from Display P3 to sRGB and written as hex, with an
alpha byte when it is not opaque. Only the roles the window uses are kept, in Rex's order of
themes. A theme without one of them stops the script, so a changed format never writes a
partial file.
"""

import json
import os
import sys

ROLES = [
    "canvas", "surfaceSubtle", "surfaceRaised", "terminalBackground",
    "strokeSubtle", "stroke",
    "content", "contentSecondary", "contentTertiary",
    "controlHover", "accent", "accentOnSolid", "accentDim",
    "danger", "dangerContainer", "success", "warning", "info",
] + [f"terminalAnsi{i}" for i in range(16)]


def decode(v):
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def encode(v):
    v = max(0.0, min(1.0, v))
    return v * 12.92 if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055


def srgb_hex(c):
    """A Display P3 color as sRGB hex: linear P3, XYZ (D65), linear sRGB, clipped."""
    if c.get("colorSpace", "display-p3") != "display-p3":
        raise SystemExit(f"unexpected color space {c.get('colorSpace')}")
    r, g, b = decode(c["red"]), decode(c["green"]), decode(c["blue"])
    x = 0.4865709 * r + 0.2656677 * g + 0.1982173 * b
    y = 0.2289746 * r + 0.6917385 * g + 0.0792869 * b
    z = 0.0451134 * g + 1.0439444 * b
    rgb = (
        3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
        -0.9692660 * x + 1.8760108 * y + 0.0415560 * z,
        0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
    )
    out = "#" + "".join(f"{round(encode(v) * 255):02x}" for v in rgb)
    alpha = c.get("alpha", 1)
    if alpha < 1:
        out += f"{round(alpha * 255):02x}"
    return out


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: import.py <Rex themes.json> <source, e.g. \"Rex Beta 0.1.0 (1829)\">")
    themes = json.load(open(sys.argv[1]))
    out = []
    for t in themes:
        semantic = t["semantic"]
        missing = [r for r in ROLES if r not in semantic]
        if missing:
            raise SystemExit(f"{t['id']} has no {', '.join(missing)}")
        if t["appearance"] not in ("light", "dark"):
            raise SystemExit(f"{t['id']} has appearance {t['appearance']}")
        out.append({
            "id": t["id"],
            "name": t["name"],
            "appearance": t["appearance"],
            "colors": {r: srgb_hex(semantic[r]) for r in ROLES},
        })
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "themes.json")
    with open(path, "w") as f:
        f.write('{\n  "source": ' + json.dumps(sys.argv[2]) + ',\n  "themes": [\n')
        for i, t in enumerate(out):
            colors = ", ".join(f'"{k}": "{v}"' for k, v in t["colors"].items())
            head = f'"id": "{t["id"]}", "name": {json.dumps(t["name"])}, "appearance": "{t["appearance"]}"'
            f.write(f'    {{ {head},\n      "colors": {{ {colors} }} }}')
            f.write(",\n" if i + 1 < len(out) else "\n")
        f.write("  ]\n}\n")
    light = sum(t["appearance"] == "light" for t in out)
    print(f"{path}: {len(out)} themes, {light} light, {len(out) - light} dark")


if __name__ == "__main__":
    main()
