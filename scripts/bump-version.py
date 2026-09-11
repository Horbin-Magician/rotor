#!/usr/bin/env python3
"""Prepare and push a native release; GitHub Actions creates the draft."""

import argparse
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def run(root, *args):
    print("$ " + " ".join(args), flush=True)
    return subprocess.run(
        args, cwd=root, check=True, text=True, stdout=subprocess.PIPE
    ).stdout.strip()


def bump(root, version, *, dry_run=False, no_push=False, remote="origin"):
    # Validate before using the version as a path/ref. xtask validates full SemVer.
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise ValueError("Expected a SemVer version, for example 3.0.1 or 3.1.0-beta.1")
    tag = f"v{version}"
    if run(root, "git", "status", "--porcelain"):
        raise ValueError("Working tree must be clean; commit or stash changes first")
    branch = run(root, "git", "symbolic-ref", "--quiet", "--short", "HEAD")
    if run(root, "git", "tag", "--list", tag):
        raise ValueError(f"Tag {tag} already exists locally")
    notes = root / "doc" / "releases" / f"{version}.md"
    if not notes.is_file() or not notes.read_text(encoding="utf-8").strip():
        raise ValueError(f"Write and commit doc/releases/{version}.md first")
    if not no_push:
        # Resolve only a configured remote, never a user-supplied URL or option.
        if remote not in run(root, "git", "remote").splitlines():
            raise ValueError(f"Unknown Git remote: {remote}")
        if run(root, "git", "ls-remote", "--tags", remote, f"refs/tags/{tag}"):
            raise ValueError(f"Tag {tag} already exists on {remote}")
    cargo = ("cargo", "run", "-p", "xtask", "--locked", "--")
    # --locked also rejects a Cargo.lock that is out of sync with the workspace.
    previous = run(root, *cargo, "version")
    if previous != version:
        print(run(root, *cargo, "set-version", version, "--dry-run"))
    else:
        print(f"Workspace is already {version}; tag the current commit without a version commit.")
    push = ("git", "push", "--atomic", remote,
            f"HEAD:refs/heads/{branch}", f"refs/tags/{tag}:refs/tags/{tag}")
    if dry_run:
        if previous != version:
            print(f"[dry-run] Update Cargo.toml and Cargo.lock, commit chore: release {tag}")
        print(f"[dry-run] Tag current HEAD as {tag}")
        if not no_push:
            print("[dry-run] " + " ".join(push))
        return
    if previous != version:
        print(run(root, *cargo, "set-version", version))
        run(root, "git", "add", "--", "Cargo.toml", "Cargo.lock")
        run(root, "git", "commit", "-m", f"chore: release {tag}")
    run(root, "git", "tag", tag)
    if no_push:
        print(f"Created local release tag {tag}. No push requested.")
        return
    try:
        run(root, *push)
    except subprocess.CalledProcessError:
        print("Push failed; local commit and tag are preserved. Resolve the remote error, then retry:", file=sys.stderr)
        print(" ".join(push), file=sys.stderr)
        raise
    print(f"Pushed {tag}. Review the native-publish-draft result and publish the draft manually; Gitee sync follows publication.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Release SemVer (without v); may match the current workspace version")
    parser.add_argument("--dry-run", action="store_true", help="Check and preview without changing files or refs")
    parser.add_argument("--no-push", action="store_true", help="Create only the local release commit and tag")
    parser.add_argument("--remote", default="origin", help="Configured remote to push (default: origin)")
    args = parser.parse_args()
    try:
        bump(ROOT, **vars(args))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release stopped: {error}\n")


if __name__ == "__main__":
    main()
