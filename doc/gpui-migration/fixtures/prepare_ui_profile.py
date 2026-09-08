"""Create a new isolated UI profile from an existing synthetic PNG.

Does not capture the desktop, change the supplied image, or launch an app.
"""
import argparse
from pathlib import Path
import shutil
import struct


def prepare(image: Path, destination: Path):
    with image.open("rb") as source:
        header = source.read(24)
    if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise ValueError("Expected a PNG with an IHDR header")
    width, height = struct.unpack(">II", header[16:24])
    if not (0 < width <= 8192 and 0 < height <= 8192):
        raise ValueError("Fixture dimensions must be between 1 and 8192 pixels")
    destination.mkdir(parents=True, exist_ok=False)
    pictures = destination / "shotter" / "default"
    pictures.mkdir(parents=True)
    shutil.copyfile(image, pictures / "1.png")
    (destination / "config.toml").write_text('''language = "1"
theme = "1"
quick_actions = '[{"id":"fixture","name":"示例操作","shortcut":"Ctrl+Shift+Y","command":"echo Rotor fixture","enabled":false}]'
quick_actions_revision = "2"
translator_engine = "custom"
translator_target_lang = "zh-CN"
translator_custom_url = "http://127.0.0.1:18765/translate?text={text}&to={to}"
''', encoding="utf-8")
    # rect and image_rect use the same monitor-space origin; the PNG itself
    # starts at (0, 0). An absent/moved monitor is handled by the native restore.
    (destination / "shotter" / "record.toml").write_text(f'''[workspaces.default.shotters.1]
monitor_pos = [0, 0]
monitor_size = [{max(1920, width + 300)}, {max(1080, height + 300)}]
rect = [300, 300, {width}, {height}]
image_rect = [300, 300, {width}, {height}]
offset = [0, 0]
zoom_factor = 100
mask_label = "ssmask-ui-fixture"
minimized = false
''', encoding="utf-8")
    print(f"Created isolated profile with one {width}x{height} pin: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path, help="An existing synthetic PNG")
    parser.add_argument("destination", type=Path, help="A new profile directory")
    arguments = parser.parse_args()
    prepare(arguments.image.resolve(strict=True), arguments.destination.resolve())
