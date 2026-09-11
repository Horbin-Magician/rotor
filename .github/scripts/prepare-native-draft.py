"""Verify downloaded candidate inventories and collect native draft attachments.

Signatures are verified by the producing Rust signing step. This module verifies
the downloaded bytes and attachment set; it never signs or publishes a release.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def collect(source: Path, destination: Path, version: str, platforms="both", production=True,
            update_manifest=None, asset_base=None) -> int:
    packages = sorted(source.iterdir())
    if not packages:
        raise ValueError("No candidate packages")
    prefix = "Rotor" if production else "Rotor-Dev"
    targets = {
        "windows": {f"{prefix}_{version}_x64-setup.exe"},
        "macos": {f"{prefix}_{version}_aarch64.dmg", f"{prefix}_{version}_aarch64.app.tar.gz"},
    }
    if platforms not in ("windows", "macos", "both"):
        raise ValueError("Unknown platform selection")
    payload_names = targets["windows"] | targets["macos"] if platforms == "both" else targets[platforms]
    metadata_names = {"native-build.json", "source.sha256"}
    allowed = payload_names | {name + ".sig" for name in payload_names} | metadata_names
    attachments = []
    output_names = set()
    payload_count = 0
    observed_payloads = set()
    source_digest = None
    signatures = {}
    for package in packages:
        if package.is_symlink() or not package.is_dir():
            raise ValueError("Expected an ordinary candidate directory")
        manifest_path = package / "resources.json"
        if manifest_path.is_symlink():
            raise ValueError("Linked inventory")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        files = manifest["files"]
        if manifest["version"] != version or not metadata_names <= files.keys():
            raise ValueError("Candidate metadata is incomplete or has a different version")
        actual = {path.name for path in package.iterdir()}
        if actual != files.keys() | {"resources.json"}:
            raise ValueError("Candidate file set differs from its inventory")
        if not (payload_names & files.keys()):
            raise ValueError("Candidate has no native payload")
        for name, expected in files.items():
            if name not in allowed or Path(name).name != name or "\\" in name:
                raise ValueError("Unexpected candidate path")
            artifact = package / name
            if artifact.is_symlink() or not artifact.is_file():
                raise ValueError("Expected an ordinary artifact file")
            data = artifact.read_bytes()
            if len(data) != expected["bytes"] or hashlib.sha256(data).hexdigest() != expected["sha256"]:
                raise ValueError("Artifact inventory mismatch")
            if name in payload_names:
                if name + ".sig" not in files:
                    raise ValueError("Missing inventoried updater signature")
                payload_count += 1
                observed_payloads.add(name)
                signatures[name] = (package / (name + ".sig")).read_text(encoding="utf-8").strip()
            elif name.endswith(".sig") and name[:-4] not in files:
                raise ValueError("Signature has no corresponding payload")
            if name in metadata_names:
                output_name = package.name + "-" + name
            else:
                output_name = name
            if output_name in output_names:
                raise ValueError("Duplicate attachment name")
            output_names.add(output_name)
            attachments.append((artifact, output_name))
        info = json.loads((package / "native-build.json").read_text(encoding="utf-8"))
        if info.get("production") is not production or info.get("identifier") != ("cc.fluctus.rotor" if production else "cc.fluctus.rotor.dev") or info.get("version") != version:
            raise ValueError("Unexpected candidate identity or version")
        attachments.append((manifest_path, package.name + "-resources.json"))
        receipt = (package / "source.sha256").read_text(encoding="utf-8")
        if len(receipt) != 64 or any(c not in "0123456789abcdef" for c in receipt):
            raise ValueError("Invalid source receipt")
        if source_digest is not None and source_digest != receipt:
            raise ValueError("Packages were built from different inputs")
        source_digest = receipt
    if observed_payloads != payload_names:
        raise ValueError("Selected platform package set is incomplete")
    if update_manifest is not None:
        manifest = json.loads(update_manifest.read_text(encoding="utf-8"))
        expected_targets = {}
        if platforms in ("windows", "both"):
            expected_targets["windows-x86_64"] = f"{prefix}_{version}_x64-setup.exe"
        if platforms in ("macos", "both"):
            expected_targets["darwin-aarch64"] = f"{prefix}_{version}_aarch64.app.tar.gz"
        if manifest.get("schema_version") != 1 or manifest.get("version") != version or set(manifest["platforms"]) != set(expected_targets):
            raise ValueError("Manifest does not match selected packages")
        if not asset_base or not asset_base.startswith("https://"):
            raise ValueError("Expected HTTPS release asset base")
        for target, name in expected_targets.items():
            entry = manifest["platforms"][target]
            if entry["signature"].strip() != signatures[name] or entry["url"] != asset_base.rstrip("/") + "/" + name:
                raise ValueError("Manifest artifact differs from verified package")
        attachments.append((update_manifest, "native-update.json"))

    # Validate every package before producing any draft attachment directory.
    destination.mkdir()
    for artifact, output_name in attachments:
        shutil.copyfile(artifact, destination / output_name)
    return payload_count


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platforms", choices=["windows", "macos", "both"], required=True)
    parser.add_argument("--development", action="store_true")
    parser.add_argument("--update-manifest", type=Path)
    parser.add_argument("--asset-base")
    args = parser.parse_args()
    print(f"Verified {collect(args.source, args.output, args.version, args.platforms, not args.development, args.update_manifest, args.asset_base)} native payloads")
