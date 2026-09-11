"""Check lockfile and supported-target dependency graphs without building UI."""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
for name in re.findall(r'^name = "([^"]+)"$', lock, re.MULTILINE):
    if name.lower().startswith("tauri"):
        raise SystemExit(f"Forbidden package in Cargo.lock: {name}")
for target in ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"]:
    tree = subprocess.check_output([
        "cargo", "tree", "--workspace", "--locked", "--target", target,
        "--edges", "normal,build", "--prefix", "none",
    ], cwd=ROOT, text=True, encoding="utf-8")
    if re.search(r"^tauri[^ ]* v", tree, re.MULTILINE | re.IGNORECASE):
        raise SystemExit(f"Forbidden package in {target} graph")
    print(f"Native dependency graph passed: {target}")
print("Cargo.lock contains no Tauri packages")
