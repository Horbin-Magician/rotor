"""Prepare release metadata locally. This script makes no network requests."""
import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import quote


def file_digest(source):
    digest = hashlib.sha256()
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(chunk)
    return digest.hexdigest()


def validate_release(release, tag):
    if release.get("tag_name") != tag or release.get("draft"):
        raise ValueError("Expected a published release with the requested tag")
    names = set()
    for asset in release.get("assets", []):
        name = asset["name"]
        if not name or name in (".", "..") or any(c in name for c in "/\\\0\r\n;,\"") or name in names:
            raise ValueError("Unsafe or duplicate release asset name")
        names.add(name)
    if not names:
        raise ValueError("Release has no assets")
    return names


def rewrite_manifest(manifest, github_repo, gitee_owner, gitee_repo):
    source = f"https://github.com/{github_repo}/releases/download/"
    destination = f"https://gitee.com/{quote(gitee_owner, safe='')}/{quote(gitee_repo, safe='')}/releases/download/"
    for artifact in manifest["platforms"].values():
        value = artifact["url"]
        if not value.startswith(source):
            raise ValueError("Update URL does not belong to the mirrored repository")
        artifact["url"] = destination + value[len(source):]
    return manifest


def prepare(release, tag, directory, github_repo, gitee_owner, gitee_repo):
    names = validate_release(release, tag)
    actual = {path.name for path in directory.iterdir()}
    if actual != names:
        raise ValueError("Downloaded asset set differs from the release")
    hashes = {}
    for asset in release["assets"]:
        path = directory / asset["name"]
        if path.is_symlink() or not path.is_file() or path.stat().st_size != asset["size"]:
            raise ValueError("Downloaded asset size or type is invalid")
        with path.open("rb") as source:
            digest = file_digest(source)
        expected = asset.get("digest")
        if expected and expected != f"sha256:{digest}":
            raise ValueError("Downloaded asset digest differs from GitHub")
        hashes[path.name] = digest
    # Prepare all changes before writing any metadata.
    changes = {}
    for name in ("native-update.json", "native-stable.json", "native-preview.json"):
        if name in names:
            path = directory / name
            value = rewrite_manifest(json.loads(path.read_text(encoding="utf-8")), github_repo, gitee_owner, gitee_repo)
            changes[name] = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    for name, value in changes.items():
        (directory / name).write_text(value, encoding="utf-8", newline="\n")
    # Signatures and binaries must remain byte-for-byte identical.
    for name, digest in hashes.items():
        if name not in changes:
            with (directory / name).open("rb") as source:
                if file_digest(source) != digest:
                    raise ValueError("Non-metadata asset changed")
    return {
        "tag_name": tag,
        "target_commitish": tag,
        "name": release.get("name") or tag,
        "body": release.get("body") or "",
        "draft": False,
        "prerelease": bool(release.get("prerelease")),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--assets", type=Path)
    parser.add_argument("--github-repo")
    parser.add_argument("--gitee-owner")
    parser.add_argument("--gitee-repo")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    release = json.loads(args.release.read_text(encoding="utf-8"))
    validate_release(release, args.tag)
    if args.assets:
        if not all([args.github_repo, args.gitee_owner, args.gitee_repo, args.output]):
            parser.error("preparation requires repository names and --output")
        payload = prepare(release, args.tag, args.assets, args.github_repo, args.gitee_owner, args.gitee_repo)
        args.output.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
