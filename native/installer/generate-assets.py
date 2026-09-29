#!/usr/bin/env python3
"""Author installer artwork and Finder layouts; not part of normal packaging.

macOS: pip install -r native/installer/requirements.txt
      python3 native/installer/generate-assets.py
Uses installed system fonts and the existing Rotor logo.
"""

import argparse
import datetime
import os
from pathlib import Path
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BLUE = "#29a6d7"
INK = "#18354b"
MUTED = "#637e90"
FONT = (str(Path(os.environ.get("WINDIR", "C:/Windows")) / "Fonts/segoeui.ttf")
        if os.name == "nt" else "/System/Library/Fonts/Supplemental/Arial.ttf")
BOLD = (str(Path(os.environ.get("WINDIR", "C:/Windows")) / "Fonts/seguisb.ttf")
        if os.name == "nt" else "/System/Library/Fonts/Supplemental/Arial Bold.ttf")
CJK = "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"
SCALE = 3


def canvas(size, color):
    return Image.new("RGB", tuple(n * SCALE for n in size), color)


def box(draw, bounds, fill, radius=0, outline=None, width=1):
    draw.rounded_rectangle(tuple(v * SCALE for v in bounds), radius * SCALE,
                           fill=fill, outline=outline, width=width * SCALE)


def text(draw, position, value, size, color=INK, font=FONT):
    draw.text(tuple(v * SCALE for v in position), value,
              font=ImageFont.truetype(font, size * SCALE), fill=color)


def centered_text(draw, width, y, value, size, color=INK, font=FONT):
    face = ImageFont.truetype(font, size * SCALE)
    left, _, right, _ = draw.textbbox((0, 0), value, font=face)
    draw.text(((width * SCALE - left - right) / 2, y * SCALE), value,
              font=face, fill=color)


def logo(image, position, size):
    mark = Image.open(ROOT / "assets/icons/icon.png").convert("RGBA")
    mark = mark.resize((size * SCALE, size * SCALE), Image.Resampling.LANCZOS)
    image.paste(mark, tuple(v * SCALE for v in position), mark)


def save_scaled(image, name, size):
    image.resize(size, Image.Resampling.LANCZOS).save(HERE / name)


def windows_artwork():
    side = canvas((164, 314), "#102b43")
    d = ImageDraw.Draw(side)
    # Paint at 3x, then downsample to the checked-in 2x MUI bitmaps.
    for y in range(side.height):
        t = y / (side.height - 1)
        color = tuple(round(a + (b - a) * t)
                      for a, b in zip((16, 43, 67), (24, 72, 95)))
        d.line((0, y, side.width, y), fill=color)
    for bounds in ((65, -72, 253, 116), (83, -54, 235, 98),
                   (-103, 244, 109, 456), (-85, 262, 91, 438)):
        d.ellipse(tuple(v * SCALE for v in bounds), outline="#23516a", width=SCALE)
    box(d, (20, 25, 72, 77), "#ffffff", 14)
    logo(side, (27, 32), 38)
    text(d, (20, 87), "Rotor", 30, "#ffffff", BOLD)
    box(d, (21, 133, 45, 136), "#53c7ef", 1)

    # Small line icons stay legible at 100% and need no additional icon font.
    for index, label in enumerate(("SEARCH", "CAPTURE", "TRANSLATE")):
        y = 161 + index * 36
        box(d, (20, y, 44, y + 24), "#24546b", 7)
        glyph = Image.new("RGBA", (24 * SCALE, 24 * SCALE))
        g = ImageDraw.Draw(glyph)
        color = "#9ee5fb"

        def line(points):
            g.line([(x * SCALE, y * SCALE) for x, y in points],
                   fill=color, width=SCALE)

        if index == 0:
            g.ellipse((6*SCALE, 5*SCALE, 15*SCALE, 14*SCALE),
                      outline=color, width=SCALE)
            line(((14, 13), (19, 18)))
        elif index == 1:
            for points in (((9, 6), (6, 6), (6, 10)),
                           ((15, 6), (18, 6), (18, 10)),
                           ((6, 14), (6, 18), (10, 18)),
                           ((18, 14), (18, 18), (14, 18))):
                line(points)
        else:
            line(((5, 8), (18, 8), (15, 5)))
            line(((19, 16), (6, 16), (9, 19)))
        side.paste(glyph, (20*SCALE, y*SCALE), glyph)
        text(d, (53, y + 6), label, 9, "#d9eff8", BOLD)
    text(d, (20, 286), "WINDOWS DESKTOP", 7, "#99c2d5", BOLD)
    save_scaled(side, "windows-sidebar.bmp", (328, 628))

    header = canvas((150, 57), "white")
    d = ImageDraw.Draw(header)
    box(d, (19, 10, 55, 46), "#eff8fc", 10)
    logo(header, (24, 15), 26)
    text(d, (65, 14), "Rotor", 22, font=BOLD)
    save_scaled(header, "windows-header.bmp", (300, 114))


def macos_artwork():
    bg = canvas((720, 300), "#f5f9fc")
    d = ImageDraw.Draw(bg)
    d.ellipse((510*SCALE, -410*SCALE, 1040*SCALE, 120*SCALE), fill="#eaf4fa")
    for left in (80, 420):
        box(d, (left, 35, left+220, 190), "white", 22, "#e1ecf3")
    # Finder supplies the real, draggable icons and their localized labels.
    for x, number in ((97, "01"), (437, "02")):
        text(d, (x, 48), number, 11, "#8da7b8", BOLD)
    d.line([(333*SCALE, 106*SCALE), (386*SCALE, 106*SCALE)], fill=BLUE, width=3*SCALE)
    d.line([(375*SCALE, 95*SCALE), (386*SCALE, 106*SCALE), (375*SCALE, 117*SCALE)],
           fill=BLUE, width=3*SCALE)
    centered_text(d, 720, 211, "拖动左侧应用到 Applications 文件夹", 19, font=CJK)
    centered_text(d, 720, 244, "Drag the app into Applications to install.", 14, MUTED)
    save_scaled(bg, "macos-background.png", (720, 300))
    with tempfile.TemporaryDirectory() as temp:
        retina = Path(temp) / "background@2x.png"
        bg.resize((1440, 600), Image.Resampling.LANCZOS).save(retina)
        subprocess.run(["tiffutil", "-cathidpicheck", str(HERE / "macos-background.png"),
                        str(retina), "-out", str(HERE / "macos-background.tiff")], check=True)


def finder_layout(product, destination):
    from ds_store import DSStore
    from mac_alias import Alias, VolumeInfo, TargetInfo, ALIAS_NO_CNID

    # Resolve by volume-relative path, never by an authoring machine's inode.
    epoch = datetime.datetime(2000, 1, 1, tzinfo=datetime.timezone.utc)
    alias = Alias(volume=VolumeInfo(product, epoch, b"H+", 5, 0, b"\0\0",
                                  posix_path=f"/Volumes/{product}"),
                  target=TargetInfo(0, "background.tiff", ALIAS_NO_CNID,
                                    ALIAS_NO_CNID, epoch, b"\0"*4, b"\0"*4,
                                    folder_name=".background",
                                    posix_path="/.background/background.tiff"))
    with DSStore.open(str(destination), "w+") as store:
        store["."]["vSrn"] = ("long", 1)
        store["."]["icvl"] = ("type", b"icnv")
        store["."]["bwsp"] = {
            "WindowBounds": "{{160, 120}, {720, 332}}",
            "ShowStatusBar": False, "ShowTabView": False, "ShowToolbar": False,
            "ShowPathbar": False, "ShowSidebar": False, "SidebarWidth": 0,
            "ContainerShowSidebar": False, "PreviewPaneVisibility": False,
        }
        store["."]["icvp"] = {
            "viewOptionsVersion": 1, "backgroundType": 2,
            "backgroundImageAlias": alias.to_bytes(),
            "backgroundColorRed": 0.96, "backgroundColorGreen": 0.98,
            "backgroundColorBlue": 0.99, "gridOffsetX": 0.0, "gridOffsetY": 0.0,
            "gridSpacing": 100.0, "arrangeBy": "none", "showIconPreview": True,
            "showItemInfo": False, "labelOnBottom": True, "textSize": 12.0,
            "iconSize": 80.0, "scrollPositionX": 0.0, "scrollPositionY": 0.0,
        }
        store[f"{product}.app"]["Iloc"] = (190, 98)
        store["Applications"]["Iloc"] = (530, 98)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("windows", "macos", "all"),
                        default="windows" if os.name == "nt" else "all")
    args = parser.parse_args()
    if args.platform in ("windows", "all"):
        windows_artwork()
    if args.platform in ("macos", "all"):
        import tomllib

        macos_artwork()
        config = tomllib.loads((ROOT / "native/app.toml").read_text())
        finder_layout(config["product_name"], HERE / "macos-development.dsstore")
        finder_layout(config["production_product_name"], HERE / "macos-production.dsstore")
