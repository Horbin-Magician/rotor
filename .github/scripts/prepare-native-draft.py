"""Verify downloaded candidate inventories and collect native draft attachments.

Signatures are verified by the producing Rust signing step. This module verifies
the downloaded bytes and attachment set; it never signs or publishes a release.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def collect(source: Path, destination: Path, version: str) -> int:
    packages = sorted(source.iterdir())
    if not packages:
        raise ValueError("No candidate packages")
    payload_names = {
        f"Rotor_{version}_x64-setup.exe",
        f"Rotor_{version}_aarch64.dmg",
        "Rotor_aarch64.app.tar.gz",
    }
    metadata_names = {"native-build.json", "source.sha256"}
    allowed = payload_names | {name + ".sig" for name in payload_names} | metadata_names
    attachments = []
    output_names = set()
    payload_count = 0
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
        if info.get("production") is not True or info.get("identifier") != "cc.fluctus.rotor" or info.get("version") != version:
            raise ValueError("Unexpected candidate identity or version")
        attachments.append((manifest_path, package.name + "-resources.json"))
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
    args = parser.parse_args()
    print(f"Verified {collect(args.source, args.output, args.version)} native payloads")
