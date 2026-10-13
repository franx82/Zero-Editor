#!/usr/bin/env python3
"""
Generates assets/ZeroWhitespace.ttf, a minimal TrueType font (<1KB)
that maps:
  - U+0020 (Space) -> centered subtle dot (advance 600, radius 38)
  - U+0009 (Tab)   -> crisp rightwards arrow (advance 600)
Used by Zero Editor for zero-runtime-cost, pixel-perfect monospace whitespace visualization.
"""

from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.fontBuilder import FontBuilder
import math
import os

def main():
    assets_dir = os.path.join(os.path.dirname(__file__), "..", "assets")
    os.makedirs(assets_dir, exist_ok=True)
    out_path = os.path.join(assets_dir, "ZeroWhitespace.ttf")

    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder([".notdef", "space", "tab"])
    fb.setupCharacterMap({0x20: "space", 0x09: "tab"})

    # .notdef glyph
    pen = TTGlyphPen(None)
    notdef_glyph = pen.glyph()

    # Space: advance 600. A centered dot at x=300, y=280 with radius 38.
    pen = TTGlyphPen(None)
    cx, cy = 300, 280
    r = 38
    points = []
    for i in range(8):
        angle = i * (2 * math.pi / 8)
        points.append((cx + r * math.cos(angle), cy + r * math.sin(angle)))

    pen.moveTo(points[0])
    for pt in points[1:]:
        pen.lineTo(pt)
    pen.closePath()
    space_glyph = pen.glyph()

    # Tab: advance 600. Right arrow ->
    pen = TTGlyphPen(None)
    # Shaft
    pen.moveTo((160, 260))
    pen.lineTo((420, 260))
    pen.lineTo((420, 300))
    pen.lineTo((160, 300))
    pen.closePath()
    # Head
    pen.moveTo((390, 200))
    pen.lineTo((490, 280))
    pen.lineTo((390, 360))
    pen.lineTo((360, 330))
    pen.lineTo((430, 280))
    pen.lineTo((360, 230))
    pen.closePath()
    tab_glyph = pen.glyph()

    fb.setupGlyf({".notdef": notdef_glyph, "space": space_glyph, "tab": tab_glyph})
    metrics = {
        ".notdef": (600, 0),
        "space": (600, 0),
        "tab": (600, 0),
    }
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({
        "familyName": "ZeroWhitespace",
        "styleName": "Regular",
        "uniqueFontIdentifier": "ZeroWhitespace-Regular",
        "fullName": "ZeroWhitespace Regular",
        "psName": "ZeroWhitespace-Regular",
        "version": "Version 1.0",
    })
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, sTypoLineGap=0, usWinAscent=800, usWinDescent=200)
    fb.setupPost()
    fb.save(out_path)
    print(f"Generated {out_path} ({os.path.getsize(out_path)} bytes)")

if __name__ == "__main__":
    main()
