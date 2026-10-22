#!/usr/bin/env python3
"""
Generates assets/icon.png and assets/icon.icns by drawing the "Frost bands" icon:
a slashed zero filled with Nord frost bands on a Polar Night squircle.
Everything is drawn on the 1024 macOS icon grid (824px squircle) at 4x and downsampled.
"""

import math
import os
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFilter

SCALE = 4
SIZE = 1024 * SCALE

NORD0 = (46, 52, 64, 255)
FROST = [
    (143, 188, 187, 255),  # nord7
    (136, 192, 208, 255),  # nord8
    (129, 161, 193, 255),  # nord9
    (94, 129, 172, 255),   # nord10
]


def s(v):
    return v * SCALE


def squircle_mask(x0, y0, size, exponent=5.0, steps=2048):
    """Superellipse mask approximating the macOS continuous-corner icon shape."""
    mask = Image.new("L", (SIZE, SIZE), 0)
    r = size / 2
    cx, cy = x0 + r, y0 + r
    points = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, sn = math.cos(t), math.sin(t)
        points.append((
            s(cx + r * math.copysign(abs(c) ** (2 / exponent), c)),
            s(cy + r * math.copysign(abs(sn) ** (2 / exponent), sn)),
        ))
    ImageDraw.Draw(mask).polygon(points, fill=255)
    return mask


def stadium(draw, x, y, w, h, fill):
    draw.rounded_rectangle([s(x), s(y), s(x + w), s(y + h)], radius=s(min(w, h) / 2), fill=fill)


def draw_icon():
    face = Image.new("RGBA", (SIZE, SIZE), NORD0)

    # Frost bands clipped to the outer zero
    bands = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    bd = ImageDraw.Draw(bands)
    edges = [232, 370, 510, 650, 792]
    for color, top, bottom in zip(FROST, edges, edges[1:]):
        bd.rectangle([s(300), s(top), s(724), s(bottom)], fill=color)
    zero = Image.new("L", (SIZE, SIZE), 0)
    stadium(ImageDraw.Draw(zero), 312, 232, 400, 560, 255)
    face.paste(bands, (0, 0), zero)

    # Counter and slash cut out in the background colour
    fd = ImageDraw.Draw(face)
    stadium(fd, 408, 328, 208, 368, NORD0)
    (x1, y1), (x2, y2), width = (372, 760), (652, 264), 44
    fd.line([s(x1), s(y1), s(x2), s(y2)], fill=NORD0, width=s(width))
    for x, y in ((x1, y1), (x2, y2)):
        fd.ellipse([s(x - width / 2), s(y - width / 2), s(x + width / 2), s(y + width / 2)], fill=NORD0)

    # Squircle with a soft drop shadow
    mask = squircle_mask(100, 100, 824)
    shadow_mask = squircle_mask(100, 110, 824).point(lambda a: a * 140 // 255)
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 120))
    shadow.putalpha(shadow_mask.filter(ImageFilter.GaussianBlur(s(18))))

    canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    canvas.alpha_composite(shadow)
    canvas.paste(face, (0, 0), mask)
    return canvas.resize((1024, 1024), Image.Resampling.LANCZOS)


def main():
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    assets_dir = os.path.join(root_dir, "assets")
    os.makedirs(assets_dir, exist_ok=True)

    canvas = draw_icon()
    png_path = os.path.join(assets_dir, "icon.png")
    canvas.save(png_path)
    print(f"Saved {png_path}")

    # Build .iconset
    iconset_dir = os.path.join(assets_dir, "icon.iconset")
    os.makedirs(iconset_dir, exist_ok=True)

    sizes = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    for filename, size in sizes:
        canvas.resize((size, size), Image.Resampling.LANCZOS).save(os.path.join(iconset_dir, filename))

    icns_path = os.path.join(assets_dir, "icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset_dir, "-o", icns_path], check=True)
    print(f"Generated {icns_path} ({os.path.getsize(icns_path)} bytes)")

    shutil.rmtree(iconset_dir)


if __name__ == "__main__":
    main()
