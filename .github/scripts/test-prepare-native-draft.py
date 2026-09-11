import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("draft", Path(__file__).with_name("prepare-native-draft.py"))
draft = importlib.util.module_from_spec(spec)
spec.loader.exec_module(draft)


class DraftTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="rotor-draft-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / "candidates"
        self.source.mkdir()
        self.output = self.root / "attachments"

    def package(self, name="windows", payload="Rotor_3.0.0_x64-setup.exe", production=True):
        directory = self.source / name
        directory.mkdir()
        (directory / payload).write_bytes(b"synthetic artifact; never executed")
        (directory / (payload + ".sig")).write_bytes(b"signature fixture; crypto checked by producer")
        (directory / "source.sha256").write_text("a" * 64)
        (directory / "native-build.json").write_text(json.dumps({
            "production": production, "identifier": "cc.fluctus.rotor", "version": "3.0.0"
        }))
        self.inventory(directory)
        return directory

    def inventory(self, directory):
        files = {}
        for path in directory.iterdir():
            if path.name == "resources.json":
                continue
            data = path.read_bytes()
            files[path.name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        (directory / "resources.json").write_text(json.dumps({"version": "3.0.0", "files": files}))

    def rejected(self):
        with self.assertRaises(ValueError):
            draft.collect(self.source, self.output, "3.0.0")
        self.assertFalse(self.output.exists())

    def test_both_platforms_keep_distinct_receipts(self):
        self.package()
        mac = self.package("macos", "Rotor_3.0.0_aarch64.app.tar.gz")
        (mac / "Rotor_3.0.0_aarch64.dmg").write_bytes(b"dmg")
        (mac / "Rotor_3.0.0_aarch64.dmg.sig").write_bytes(b"signature")
        self.inventory(mac)
        self.assertEqual(draft.collect(self.source, self.output, "3.0.0"), 3)
        self.assertTrue((self.output / "windows-resources.json").is_file())
        self.assertTrue((self.output / "macos-native-build.json").is_file())

    def test_windows_only_and_manifest_must_match(self):
        self.package()
        update = self.root / "native-update.json"
        base = "https://example.com/releases/download/v3.0.0"
        entry = {"schema_version": 1, "version": "3.0.0", "platforms": {
            "windows-x86_64": {"signature": "signature fixture; crypto checked by producer",
                               "url": base + "/Rotor_3.0.0_x64-setup.exe"}}}
        update.write_text(json.dumps(entry))
        self.assertEqual(draft.collect(self.source, self.output, "3.0.0", "windows", True, update, base), 1)
        self.assertTrue((self.output / "native-update.json").is_file())
        entry["platforms"]["darwin-aarch64"] = entry["platforms"]["windows-x86_64"]
        update.write_text(json.dumps(entry))
        with self.assertRaises(ValueError):
            draft.collect(self.source, self.root / "invalid", "3.0.0", "windows", True, update, base)

    def test_missing_selected_platform_is_rejected(self):
        self.package()
        self.rejected()

    def test_tampering_rejects_before_copying_any_package(self):
        self.package("a-valid")
        bad = self.package("z-bad", "Rotor_3.0.0_aarch64.app.tar.gz")
        (bad / "Rotor_3.0.0_aarch64.app.tar.gz").write_bytes(b"modified")
        self.rejected()

    def test_uninventoried_signature_and_missing_signature_are_rejected(self):
        directory = self.package()
        signature = directory / "Rotor_3.0.0_x64-setup.exe.sig"
        data = signature.read_bytes()
        signature.unlink()
        self.inventory(directory)
        self.rejected()
        signature.write_bytes(data)
        self.rejected()

    def test_duplicate_payloads_and_wrong_identity_are_rejected(self):
        first = self.package(production=False)
        self.rejected()
        info = json.loads((first / "native-build.json").read_text())
        info["production"] = True
        (first / "native-build.json").write_text(json.dumps(info))
        self.inventory(first)
        self.package("duplicate")
        self.rejected()

    def test_unexpected_paths_and_extra_files_are_rejected(self):
        directory = self.package()
        (directory / "unexpected.txt").write_text("untrusted")
        self.rejected()
        self.inventory(directory)
        self.rejected()


if __name__ == "__main__":
    unittest.main()
