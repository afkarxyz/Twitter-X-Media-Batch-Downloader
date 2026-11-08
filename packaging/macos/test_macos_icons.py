"""Cross-platform contracts for original Icon Composer source and compiler outputs."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import macos_icons


class MacIconTests(unittest.TestCase):
    def test_original_project_references_existing_local_artwork(self):
        source = macos_icons.validate_source()
        self.assertEqual(source.name, "twitter-dl.icon")
        self.assertTrue((source / "Assets/logo.png").is_file())

    def test_original_import_matches_recorded_hashes(self):
        import hashlib
        manifest = json.loads((macos_icons.ROOT / "packaging/macos/icon-source.json").read_text())
        source = macos_icons.validate_source()
        self.assertEqual(source.name, manifest["project"])
        self.assertEqual({p.relative_to(source).as_posix() for p in source.rglob("*") if p.is_file()}, set(manifest["sha256"]))
        for name, digest in manifest["sha256"].items():
            self.assertEqual(hashlib.sha256((source / name).read_bytes()).hexdigest(), digest)

    def test_missing_or_escaping_artwork_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "packaging/macos/twitter-dl.icon"
            (project / "Assets").mkdir(parents=True)
            for name in ("missing.svg", "../../outside.svg"):
                (project / "icon.json").write_text(json.dumps({"groups": [{"layers": [{"image-name": name}]}]}))
                with patch.object(macos_icons, "ROOT", root), self.assertRaises(ValueError):
                    macos_icons.validate_source()

    def test_older_xcode_cannot_silently_produce_a_static_only_release(self):
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(macos_icons.subprocess, "check_output", return_value="Xcode 16.4\nBuild version fixture"), \
                patch.object(macos_icons.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "Xcode 26"):
                macos_icons.build(Path(directory), Path(directory))
            run.assert_not_called()

    def test_compiler_output_preserves_legacy_support_and_bundle_icon_name(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resources = root / "Resources"
            resources.mkdir()
            work = root / "work"
            work.mkdir()
            def run(command, **kwargs):
                if command[0] == "sips":
                    Path(command[-1]).write_bytes(b"fixture raster")
                elif command[0] == "iconutil":
                    Path(command[-1]).write_bytes(b"fixture icns")
                elif command[:2] == ["xcrun", "actool"]:
                    catalog = work / "LegacyIcons.xcassets/AppIcon.appiconset/Contents.json"
                    entries = json.loads(catalog.read_text())["images"]
                    self.assertEqual(len(entries), 10)
                    self.assertTrue(all(v["idiom"] == "mac" for v in entries))
                    self.assertIn("--minimum-deployment-target=11.0", command)
                    self.assertIn("--app-icon=AppIcon", command)
                    self.assertEqual(command[-1], str(work / "AppIcon.icon"))
                    (work / "compiled-icons/Assets.car").write_bytes(b"fixture compiled catalog")
                return subprocess.CompletedProcess(command, 0)
            with patch.object(macos_icons.subprocess, "check_output", return_value="Xcode 26.1\nBuild version fixture"), \
                    patch.object(macos_icons.subprocess, "run", side_effect=run):
                info = macos_icons.build(resources, work)
            self.assertEqual(info, {"CFBundleIconFile": "AppIcon", "CFBundleIconName": "AppIcon"})
            self.assertEqual((resources / "Assets.car").read_bytes(), b"fixture compiled catalog")
            self.assertTrue((resources / "AppIcon.icns").is_file())
            original = macos_icons.validate_source()
            self.assertEqual((work / "AppIcon.icon/icon.json").read_bytes(), (original / "icon.json").read_bytes())


if __name__ == "__main__":
    unittest.main()
