"""Installer safety tests; only temporary app directories are modified."""
import plistlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import macos_app


class InstallerTests(unittest.TestCase):
    def app(self, parent, identifier=macos_app.IDENTIFIER, marker="new"):
        app = parent / macos_app.APP_NAME
        (app / "Contents").mkdir(parents=True)
        with (app / "Contents/Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": identifier}, file)
        (app / "marker").write_text(marker)
        return app

    def test_replaces_only_owned_app(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.app(root / "build")
            target = self.app(root / "Applications", marker="old")
            macos_app.replace_app(source, target)
            self.assertEqual((target / "marker").read_text(), "new")
            self.assertEqual(len(list(target.parent.iterdir())), 1)

    def test_unrelated_app_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.app(root / "build")
            target = self.app(root / "Applications", identifier="another.app", marker="other")
            with self.assertRaises(RuntimeError):
                macos_app.replace_app(source, target)
            self.assertEqual((target / "marker").read_text(), "other")
            self.assertTrue(source.exists())

    def test_failed_replacement_restores_previous_install(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = self.app(root / "Applications", marker="old")
            with self.assertRaises(FileNotFoundError):
                macos_app.replace_app(root / "missing.app", target)
            self.assertEqual((target / "marker").read_text(), "old")

    def test_symlink_target_is_not_followed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real = self.app(root / "elsewhere", marker="other")
            source = self.app(root / "build")
            target = root / macos_app.APP_NAME
            target.symlink_to(real, target_is_directory=True)
            with self.assertRaises(RuntimeError):
                macos_app.replace_app(source, target)
            self.assertEqual((real / "marker").read_text(), "other")

    def test_broken_symlink_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.app(root / "build")
            target = root / macos_app.APP_NAME
            target.symlink_to(root / "missing")
            with self.assertRaises(RuntimeError):
                macos_app.replace_app(source, target)
            self.assertTrue(target.is_symlink())
            self.assertTrue(source.exists())

    def test_icon_contains_required_resolution_representations(self):
        source = macos_app.ROOT / "assets/app-icon/photosuite.png"
        before = source.read_bytes()
        with tempfile.TemporaryDirectory() as temp:
            resources = Path(temp)
            macos_app.build_icon(resources)
            icon = resources / "PhotoSuite.icns"
            self.assertEqual(icon.read_bytes()[:4], b"icns")
            extracted = resources / "Extracted.iconset"
            macos_app.run(["/usr/bin/iconutil", "--convert", "iconset", "--output", extracted, icon])
            for size in [16, 32, 128, 256, 512]:
                for scale in [1, 2]:
                    suffix = "@2x" if scale == 2 else ""
                    self.assertTrue((extracted / f"icon_{size}x{size}{suffix}.png").is_file())
        self.assertEqual(source.read_bytes(), before)

    def test_install_copies_entire_package(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.app(root / "build")
            resource = source / "Contents/Resources/exiftool/lib/Image/test.pm"
            resource.parent.mkdir(parents=True)
            resource.write_text("resource")
            with patch.object(macos_app, "verify_bundle"), patch.object(macos_app, "ensure_closed"):
                installed = macos_app.install(source, root / "Applications", register=False)
            self.assertEqual((installed / "Contents/Resources/exiftool/lib/Image/test.pm").read_text(), "resource")
            self.assertTrue(source.exists())


if __name__ == "__main__":
    unittest.main()
