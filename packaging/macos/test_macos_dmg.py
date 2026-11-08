"""Packaging contracts that run without mounting disks or requiring Finder."""
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import macos_dmg


class MacDiskImageTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="macos-dmg-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.app = self.root / "TwitterXMediaBatchDownloader.app"
        (self.app / "Contents/Resources").mkdir(parents=True)
        (self.app / "Contents/Resources/AppIcon.icns").write_bytes(b"fixture icon")
        self.asset = self.root / "TwitterXMediaBatchDownloader-test.dmg"

    def test_layout_uses_logical_retina_size_and_only_the_signed_application(self):
        (self.root / "unrelated-private-file").write_text("must not be included")
        settings = macos_dmg.settings(self.app)
        self.assertEqual(settings["files"], [str(self.app)])
        self.assertEqual(settings["symlinks"], {"Applications": "/Applications"})
        self.assertEqual(settings["window_rect"][1], (720, 460))
        self.assertEqual(macos_dmg.png_size(settings["background"]), (720, 460))
        self.assertEqual(macos_dmg.png_size(Path(settings["background"]).with_name("dmg-background@2x.png")), (1440, 920))
        self.assertEqual(set(settings["icon_locations"]), {"TwitterXMediaBatchDownloader.app", "Applications"})
        for x, y in settings["icon_locations"].values():
            self.assertGreater(x - settings["icon_size"] / 2, 0)
            self.assertLess(x + settings["icon_size"] / 2, 720)
            self.assertGreater(y - settings["icon_size"] / 2, 150)
            self.assertLess(y + settings["icon_size"] / 2 + settings["text_size"] * 2, 380)

    def test_mismatched_background_size_is_rejected_before_disk_operations(self):
        with patch.object(macos_dmg, "png_size", return_value=(1440, 920)):
            with self.assertRaisesRegex(ValueError, "dimensions"):
                macos_dmg.settings(self.app)

    def test_mounted_verification_failure_still_detaches_the_owned_image(self):
        library = types.SimpleNamespace(build_dmg=lambda *args, **kwargs: None)
        with patch.dict(sys.modules, {"dmgbuild": library}), \
                patch.object(macos_dmg.subprocess, "run") as run, \
                patch.object(macos_dmg, "verify_mounted_layout", side_effect=ValueError("fixture invalid layout")):
            with self.assertRaisesRegex(ValueError, "invalid layout"):
                macos_dmg.build(self.app, self.asset, self.root)
            commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands[0], ["hdiutil", "verify", str(self.asset)])
        self.assertEqual(commands[-1], ["hdiutil", "detach", str(self.root / "verify-dmg")])
        self.assertIn("-readonly", commands[1])
        self.assertIn("-noautoopen", commands[1])

    def test_failed_disk_creation_never_attempts_to_mount_or_detach(self):
        def fail(*args, **kwargs):
            raise RuntimeError("fixture build failed")
        with patch.dict(sys.modules, {"dmgbuild": types.SimpleNamespace(build_dmg=fail)}), \
                patch.object(macos_dmg.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "build failed"):
                macos_dmg.build(self.app, self.asset, self.root)
            run.assert_not_called()


    def test_invalid_bundle_name_and_truncated_png_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "signed"):
            macos_dmg.settings(self.root)
        truncated = self.root / "truncated.png"
        truncated.write_bytes(b"\x89PNG\r\n\x1a\n")
        with self.assertRaisesRegex(ValueError, "Invalid"):
            macos_dmg.png_size(truncated)

    def test_detach_failure_only_force_detaches_the_owned_verification_mount(self):
        import subprocess
        commands = []
        def run(command, **kwargs):
            commands.append(command)
            if command[:2] == ["hdiutil", "detach"] and "-force" not in command:
                raise subprocess.CalledProcessError(1, command)
        with patch.dict(sys.modules, {"dmgbuild": types.SimpleNamespace(build_dmg=lambda *a, **k: None)}), \
                patch.object(macos_dmg.subprocess, "run", side_effect=run), \
                patch.object(macos_dmg, "verify_mounted_layout"):
            macos_dmg.build(self.app, self.asset, self.root)
        self.assertEqual(commands[-1], ["hdiutil", "detach", "-force", str(self.root / "verify-dmg")])


if __name__ == "__main__":
    unittest.main()
