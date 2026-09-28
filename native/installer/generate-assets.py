#!/usr/bin/env python3
"""Author installer artwork and Finder layouts; not part of normal packaging.

macOS: pip install -r native/installer/requirements.txt
      python3 native/installer/generate-assets.py
Uses installed system fonts and the existing Rotor logo.
"""

import datetime
from pathlib import Path
import subprocess
import tempfile
import tomllib

from PIL import Image, ImageDraw, ImageFont
from ds_store import DSStore
from mac_alias import Alias, VolumeInfo, TargetInfo, ALIAS_NO_CNID

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BLUE = "#29a6d7"
INK = "#18354b"
MUTED = "#637e90"
FONT = "/System/Library/Fonts/Supplemental/Arial.ttf"
BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
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


def artwork():
    side = canvas((164, 314), "#eff8fc")
    d = ImageDraw.Draw(side)
    box(d, (20, 29, 76, 85), "white", 16)
    logo(side, (28, 37), 40)
    text(d, (20, 101), "Rotor", 29, font=BOLD)
    box(d, (21, 147, 49, 150), BLUE, 1)
    # Quiet, oversized orbit shapes echo the curves in Rotor's existing mark.
    for bounds, color in [((35, 194, 249, 408), "#dfeff7"),
                           ((57, 216, 227, 386), "#eff8fc"),
                           ((86, 245, 198, 357), "#d1eaf5")]:
        d.ellipse(tuple(v * SCALE for v in bounds), fill=color)
    text(d, (20, 274), "YOUR DESKTOP.", 8, MUTED, BOLD)
    text(d, (20, 288), "WITH A LITTLE MORE FLOW.", 6, MUTED)
    save_scaled(side, "windows-sidebar.bmp", (328, 628))

    header = canvas((150, 57), "white")
    logo(header, (27, 13), 30)
    text(ImageDraw.Draw(header), (66, 17), "Rotor", 21, font=BOLD)
    save_scaled(header, "windows-header.bmp", (300, 114))

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
    artwork()
    config = tomllib.loads((ROOT / "native/app.toml").read_text())
    finder_layout(config["product_name"], HERE / "macos-development.dsstore")
    finder_layout(config["production_product_name"], HERE / "macos-production.dsstore")
