"""Exercise release refs against disposable repositories; never access a service."""

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("bump_version", Path(__file__).with_name("bump-version.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        # Ignore user signing settings, hooks and identities.
        self.env = patch.dict(os.environ, {
            "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Release Test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
            "GIT_COMMITTER_NAME": "Release Test", "GIT_COMMITTER_EMAIL": "test@example.invalid",
        })
        self.env.start()
        self.addCleanup(self.env.stop)
        self.root = self.base / "checkout"
        self.root.mkdir()
        self.remote = self.base / "remote.git"
        self.git("init", "--bare", str(self.remote))
        self.git("init", "-b", "master")
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "3.0.0"\n')
        (self.root / "Cargo.lock").write_text('version = "3.0.0"\n')
        notes = self.root / "doc/releases"
        notes.mkdir(parents=True)
        (notes / "3.0.1.md").write_text("Reviewed release notes\n")
        self.git("add", ".")
        self.git("commit", "-m", "test: initialize fixture")
        self.git("remote", "add", "origin", str(self.remote))
        self.git("push", "origin", "master")
        self.head = self.git("rev-parse", "HEAD")
        self.real_run = release.run

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True, stderr=subprocess.DEVNULL).strip()

    def fake_cargo(self, root, *args):
        if args[0] != "cargo":
            return self.real_run(root, *args)
        if args[-1] == "version":
            return "3.0.0"
        if args[-1] != "--dry-run":
            for name in ("Cargo.toml", "Cargo.lock"):
                path = root / name
                path.write_text(path.read_text().replace("3.0.0", args[-1]))
        return "Version fixture updated"

    def bump(self, **kwargs):
        with patch.object(release, "run", side_effect=self.fake_cargo), contextlib.redirect_stdout(io.StringIO()):
            release.bump(self.root, kwargs.pop("version", "3.0.1"), **kwargs)

    def test_pushes_branch_and_only_release_tag(self):
        self.git("tag", "unrelated")
        self.bump()
        commit = self.git("rev-parse", "HEAD")
        self.assertNotEqual(commit, self.head)
        self.assertEqual(self.git("log", "-1", "--format=%s"), "chore: release v3.0.1")
        self.assertEqual(self.git("rev-parse", "v3.0.1"), commit)
        refs = self.git("ls-remote", "origin")
        self.assertIn(f"{commit}\trefs/heads/master", refs)
        self.assertIn(f"{commit}\trefs/tags/v3.0.1", refs)
        self.assertNotIn("unrelated", refs)
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertIn("3.0.1", (self.root / "Cargo.lock").read_text())

    def test_no_push_works_without_remote(self):
        self.git("remote", "remove", "origin")
        self.bump(no_push=True)
        self.assertEqual(self.git("rev-parse", "v3.0.1"), self.git("rev-parse", "HEAD"))

    def test_dry_run_preserves_files_and_refs(self):
        self.bump(dry_run=True)
        self.assertEqual(self.git("rev-parse", "HEAD"), self.head)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_rejects_dirty_worktree(self):
        (self.root / "unrelated.txt").write_text("User work")
        with self.assertRaisesRegex(ValueError, "clean"):
            self.bump()
        self.assertEqual(self.git("rev-parse", "HEAD"), self.head)

    def test_rejects_local_tag(self):
        self.git("tag", "v3.0.1")
        with self.assertRaisesRegex(ValueError, "locally"):
            self.bump()

    def test_rejects_remote_tag_before_editing(self):
        self.git("tag", "v3.0.1")
        self.git("push", "origin", "refs/tags/v3.0.1")
        self.git("tag", "-d", "v3.0.1")
        with self.assertRaisesRegex(ValueError, "already exists on"):
            self.bump()
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_rejects_missing_notes_and_invalid_version(self):
        with self.assertRaisesRegex(ValueError, "Write and commit"):
            self.bump(version="3.0.2")
        with self.assertRaisesRegex(ValueError, "SemVer"):
            self.bump(version="../bad")

    def test_rejects_detached_head(self):
        self.git("checkout", "--detach")
        with self.assertRaises(subprocess.CalledProcessError):
            self.bump()

    def prepare_current_version_notes(self):
        (self.root / "doc/releases/3.0.0.md").write_text("Current release\n")
        self.git("add", ".")
        self.git("commit", "-m", "test: add current release notes")
        return self.git("rev-parse", "HEAD")

    def test_publishes_unchanged_version_without_extra_commit(self):
        head = self.prepare_current_version_notes()
        self.bump(version="3.0.0")
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("rev-parse", "v3.0.0"), head)
        self.assertIn(f"{head}\trefs/tags/v3.0.0", self.git("ls-remote", "origin"))
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_unchanged_version_dry_run_preserves_refs(self):
        head = self.prepare_current_version_notes()
        self.bump(version="3.0.0", dry_run=True)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")

    def test_unchanged_version_no_push(self):
        head = self.prepare_current_version_notes()
        self.bump(version="3.0.0", no_push=True)
        self.assertEqual(self.git("rev-parse", "v3.0.0"), head)
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")

    def test_lock_validation_failure_creates_no_tag(self):
        head = self.prepare_current_version_notes()

        def fail_locked_version(root, *args):
            if args[0] == "cargo":
                self.assertIn("--locked", args)
                raise subprocess.CalledProcessError(1, args)
            return self.real_run(root, *args)

        with patch.object(release, "run", side_effect=fail_locked_version), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError):
                release.bump(self.root, "3.0.0")
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("tag", "--list"), "")

    def test_failed_version_update_creates_no_commit_or_tag(self):
        def fail_update(root, *args):
            if args[0] == "cargo" and args[-2:] == ("set-version", "3.0.1"):
                raise subprocess.CalledProcessError(1, args)
            return self.fake_cargo(root, *args)

        with patch.object(release, "run", side_effect=fail_update), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError):
                release.bump(self.root, "3.0.1")
        self.assertEqual(self.git("rev-parse", "HEAD"), self.head)
        self.assertEqual(self.git("tag", "--list"), "")

    def test_atomic_failure_preserves_local_release_without_remote_tag(self):
        # Move the remote branch independently to make our branch push fail.
        self.git("commit", "--allow-empty", "-m", "test: advance remote")
        self.git("push", "origin", "master")
        remote_head = self.git("rev-parse", "HEAD")
        self.git("reset", "--hard", self.head)
        with self.assertRaises(subprocess.CalledProcessError), contextlib.redirect_stderr(io.StringIO()):
            self.bump()
        self.assertEqual(self.git("rev-parse", "v3.0.1"), self.git("rev-parse", "HEAD"))
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")
        self.assertIn(remote_head, self.git("ls-remote", "--heads", "origin"))


if __name__ == "__main__":
    unittest.main()
