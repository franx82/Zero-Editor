#!/usr/bin/env python3
"""
Generates assets/icon.png and assets/icon.icns from the high-res icon source.
Produces a pixel-perfect, native macOS HIG squircle icon with retina resolutions.
"""

import os
import subprocess
from PIL import Image, ImageDraw, ImageFilter

def main():
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    assets_dir = os.path.join(root_dir, "assets")
    os.makedirs(assets_dir, exist_ok=True)

    src_path = "/Users/fippoliti/.gemini/antigravity/brain/35d751e3-802c-4d37-a4ff-9292c8216b4e/zero_editor_icon_1791895118299.jpg"
    if not os.path.exists(src_path):
        src_path = os.path.join(assets_dir, "icon.png")

    img = Image.open(src_path).convert("RGBA")

    # Crop squircle
    box = (130, 130, 894, 894)
    cropped = img.crop(box)

    # Scale squircle to standard macOS HIG grid (824x824 on 1024x1024)
    target_size = 824
    squircle = cropped.resize((target_size, target_size), Image.Resampling.LANCZOS)

    # Smooth squircle mask (continuous curvature corner radius ~185)
    mask = Image.new("L", (target_size, target_size), 0)
    draw = ImageDraw.Draw(mask)
    radius = 185
    draw.rounded_rectangle([(0, 0), (target_size, target_size)], radius=radius, fill=255)

    masked_squircle = Image.new("RGBA", (target_size, target_size), (0, 0, 0, 0))
    masked_squircle.paste(squircle, (0, 0), mask)

    # 1024x1024 Canvas with soft drop shadow
    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    offset_x = (1024 - target_size) // 2
    offset_y = (1024 - target_size) // 2 + 10

    shadow = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    shadow_mask = Image.new("L", (1024, 1024), 0)
    shadow_draw = ImageDraw.Draw(shadow_mask)
    shadow_draw.rounded_rectangle(
        [(offset_x, offset_y), (offset_x + target_size, offset_y + target_size)],
        radius=radius,
        fill=140
    )
    shadow_blur = shadow_mask.filter(ImageFilter.GaussianBlur(18))
    shadow.paste((0, 0, 0, 120), (0, 0), shadow_blur)

    canvas.paste(shadow, (0, 0), shadow)
    canvas.paste(masked_squircle, (offset_x, offset_y - 10), masked_squircle)

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

    for filename, s in sizes:
        resized = canvas.resize((s, s), Image.Resampling.LANCZOS)
        resized.save(os.path.join(iconset_dir, filename))

    icns_path = os.path.join(assets_dir, "icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset_dir, "-o", icns_path], check=True)
    print(f"Generated {icns_path} ({os.path.getsize(icns_path)} bytes)")

    # Cleanup temporary iconset
    for f in os.listdir(iconset_dir):
        os.remove(os.path.join(iconset_dir, f))
    os.rmdir(iconset_dir)

if __name__ == "__main__":
    main()
