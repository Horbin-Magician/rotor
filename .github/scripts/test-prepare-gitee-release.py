import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("prepare", Path(__file__).with_name("prepare-gitee-release.py"))
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class ReleaseTests(unittest.TestCase):
    def test_rewrites_both_feeds_without_changing_signed_assets_or_notes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signed = b"signed\x00binary\xff"
            (root / "Rotor.exe").write_bytes(signed)
            (root / "Rotor.exe.sig").write_bytes(b"signature\n")
            manifest = {"notes": "line 1\n\"quoted\" $()", "platforms": {"windows-x86_64": {"signature": "same", "url": "https://github.com/owner/repo/releases/download/v2.7.0/Rotor.exe"}}}
            for name in ("native-update.json", "native-stable.json", "native-preview.json"):
                (root / name).write_text(json.dumps(manifest), encoding="utf-8")
            release = {"tag_name": "v2.7.0", "draft": False, "name": "quoted \"release\"", "body": "one\ntwo ' $()", "prerelease": True,
                       "assets": [{"name": path.name, "size": path.stat().st_size} for path in root.iterdir()]}
            result = prepare.prepare(release, "v2.7.0", root, "owner/repo", "mirror", "repo")
            self.assertEqual(result["body"], release["body"])
            self.assertTrue(result["prerelease"])
            self.assertEqual((root / "Rotor.exe").read_bytes(), signed)
            self.assertEqual((root / "Rotor.exe.sig").read_bytes(), b"signature\n")
            for name in ("native-update.json", "native-stable.json", "native-preview.json"):
                updated = json.loads((root / name).read_text(encoding="utf-8"))
                self.assertEqual(updated["notes"], manifest["notes"])
                self.assertEqual(updated["platforms"]["windows-x86_64"]["signature"], "same")
                self.assertIn("https://gitee.com/mirror/repo/", updated["platforms"]["windows-x86_64"]["url"])

    def test_rejects_wrong_tags_drafts_and_unsafe_assets(self):
        for release in [
            {"tag_name": "other", "assets": []},
            {"tag_name": "v1", "draft": True, "assets": []},
            {"tag_name": "v1", "assets": [{"name": "../escape"}]},
            {"tag_name": "v1", "assets": [{"name": "same"}, {"name": "same"}]},
        ]:
            with self.assertRaises(ValueError):
                prepare.validate_release(release, "v1")

    def test_corrupted_download_fails_before_metadata_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "latest.json").write_text("original", encoding="utf-8")
            release = {"tag_name": "v1", "assets": [{"name": "latest.json", "size": 9}]}
            with self.assertRaises(ValueError):
                prepare.prepare(release, "v1", root, "owner/repo", "mirror", "repo")
            self.assertEqual((root / "latest.json").read_text(), "original")


if __name__ == "__main__":
    unittest.main()
