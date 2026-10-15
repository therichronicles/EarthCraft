"""Offline checks for native package metadata and the complete release contract."""

from pathlib import Path
import hashlib
import tempfile
import unittest
from unittest import mock
import xml.etree.ElementTree as ET

import package_linux as linux
import package_release as release
import package_windows as windows


class NativePackagingTests(unittest.TestCase):
    def test_appimage_collects_zsync_from_tool_working_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "AppDir"
            binary = root / "binary"
            binary.write_bytes(b"executable fixture")
            source = root / "source"
            linux.stage_application(source, binary)
            linux.stage_application(directory, binary)
            output = root / "dist"
            output.mkdir()
            archive = output / "earthcraft-0.1.0-linux-x64.AppImage"
            delta = archive.with_suffix(".AppImage.zsync")
            content = b"zsync metadata fixture"

            def linuxdeploy(command, **options):
                if "--output" in command:
                    desktop = directory / f"{linux.APP_ID}.desktop"
                    metadata = directory / f"usr/share/metainfo/{desktop.stem}.appdata.xml"
                    component = ET.parse(metadata)
                    self.assertEqual(component.findtext("id"), desktop.stem)
                    self.assertEqual(component.findtext("launchable"), desktop.name)
                    self.assertEqual(desktop.read_bytes(), (source / "usr/share/applications/earthcraft.desktop").read_bytes())
                    self.assertFalse((directory / f"usr/share/metainfo/{linux.APP_ID}.metainfo.xml").exists())
                    self.assertFalse((directory / "usr/share/applications/earthcraft.desktop").exists())
                    self.assertEqual(ET.parse(source / f"usr/share/metainfo/{linux.APP_ID}.metainfo.xml").findtext("launchable"), "earthcraft.desktop")
                    destination = Path(options["env"]["LDAI_OUTPUT"])
                    destination.write_bytes(b"AppImage fixture")
                    # Model zsyncmake's basename output in the tool's cwd.
                    working_directory = Path(options.get("cwd", root))
                    (working_directory / (destination.name + ".zsync")).write_bytes(content)
                else:
                    desktop = Path(command[command.index("--desktop-file") + 1])
                    self.assertEqual(desktop.name, f"{linux.APP_ID}.desktop")
                    # Model linuxdeploy copying the supplied desktop entry.
                    (directory / desktop.name).write_bytes(desktop.read_bytes())

            with mock.patch.object(linux.subprocess, "run", side_effect=linuxdeploy):
                linux.appimage(directory, archive, "x86_64", source)
            self.assertEqual(delta.read_bytes(), content)
            self.assertEqual(delta.with_suffix(".zsync.sha256").read_text(),
                             f"{hashlib.sha256(content).hexdigest()}  {delta.name}\n")
            delta.unlink()
            linux.stage_application(directory, binary)
            with mock.patch.object(linux.subprocess, "run"):
                with self.assertRaisesRegex(ValueError, "did not create its zsync"):
                    linux.appimage(directory, archive, "x86_64", source)

    def test_complete_asset_set_requires_each_format_and_valid_checksums(self):
        version = release.workspace_version()
        expected = release.expected_assets(version)
        self.assertEqual(len(expected), 22)
        required = {
            f"earthcraft-{version}-freebsd-x64.tar.gz",
            f"earthcraft-{version}-windows-x64.exe",
            f"earthcraft-{version}-macos-arm64.dmg",
            f"earthcraft-{version}-macos-arm64-update.zip",
        }
        for architecture in ("x64", "arm64"):
            for extension in ("tar.gz", "AppImage", "AppImage.zsync", "deb", "rpm", "flatpak", "snap"):
                required.add(f"earthcraft-{version}-linux-{architecture}.{extension}")
        self.assertTrue(required <= expected)
        self.assertFalse(any("windows-x86" in name for name in expected))
        self.assertFalse(any("macos-x64" in name or "universal" in name for name in expected))
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            for name in expected:
                archive = output / name
                archive.write_bytes(name.encode())
                release.write_checksum(archive)
            release.verify_assets(version, output)
            missing = output / f"earthcraft-{version}-linux-arm64.snap"
            original = missing.read_bytes()
            missing.unlink()
            with self.assertRaisesRegex(ValueError, "missing="):
                release.verify_assets(version, output)
            missing.write_bytes(original + b"corrupted")
            with self.assertRaisesRegex(ValueError, "Checksum verification failed"):
                release.verify_assets(version, output)
            missing.write_bytes(original)
            (output / "accidental-build-output.txt").write_text("not a release asset")
            with self.assertRaisesRegex(ValueError, "unexpected="):
                release.verify_assets(version, output)

    def test_linux_packages_include_executable_desktop_icon_and_license(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            binary.write_bytes(b"executable fixture")
            staged = root / "staged"
            files = linux.stage_application(staged, binary)
            self.assertEqual((staged / "usr/bin/earthcraft").read_bytes(), binary.read_bytes())
            self.assertEqual((staged / "usr/share/licenses/earthcraft/LICENSE").read_bytes(), (release.ROOT / "LICENSE").read_bytes())
            self.assertIn("Exec=earthcraft", (staged / "usr/share/applications/earthcraft.desktop").read_text())
            metadata = ET.parse(staged / f"usr/share/metainfo/{linux.APP_ID}.metainfo.xml").getroot()
            self.assertEqual(metadata.findtext("id"), linux.APP_ID)
            self.assertEqual(metadata.findtext("launchable"), "earthcraft.desktop")
            for architecture in linux.ARCHITECTURES.values():
                config = linux.nfpm_configuration("0.1.0", architecture.package, staged, files)
                self.assertEqual(config["arch"], architecture.package)
                for content in config["contents"]:
                    self.assertTrue(Path(content["src"]).is_file())
                    self.assertTrue(content["dst"].startswith("/usr/"))
                self.assertIn("libgtk-3-0", config["overrides"]["deb"]["depends"])
                self.assertIn("gtk3", config["overrides"]["rpm"]["depends"])
                snap = linux.snap_configuration("0.1.0", architecture.package, staged)
                self.assertEqual(snap["architectures"][0]["build-for"], [architecture.package])
                self.assertEqual(snap["apps"]["earthcraft"]["environment"]["EARTHCRAFT_UPDATE_MODE"], "manual")
                self.assertIn("gnome", snap["apps"]["earthcraft"]["extensions"])
                self.assertIn("network", snap["apps"]["earthcraft"]["plugs"])

    def test_msi_upgrade_families_are_stable_and_separate_by_architecture(self):
        namespace = {"w": windows.WIX_NAMESPACE}
        binary = Path("/temporary/example/earthcraft.exe")
        codes = set()
        for architecture in windows.ARCHITECTURES.values():
            first = ET.fromstring(windows.installer_source("0.1.0", architecture, binary))
            second = ET.fromstring(windows.installer_source("0.2.0", architecture, binary))
            package = first.find("w:Package", namespace)
            self.assertEqual(package.get("Scope"), "perMachine")
            code = package.get("UpgradeCode")
            codes.add(code)
            self.assertEqual(code, second.find("w:Package", namespace).get("UpgradeCode"))
            file = first.find(".//w:File", namespace)
            self.assertEqual(file.get("Source"), str(binary.resolve()))
            shortcut = file.find("w:Shortcut", namespace)
            self.assertEqual(shortcut.get("Directory"), "ProgramMenuFolder")
            self.assertEqual(shortcut.get("Advertise"), "yes")
        self.assertEqual(len(codes), 2)
        prerelease = ET.fromstring(windows.installer_source("1.2.3-rc.1+build.4", "x64", binary))
        self.assertEqual(prerelease.find("w:Package", namespace).get("Version"), "1.2.3")
        with self.assertRaisesRegex(ValueError, "MSI versions"):
            windows.installer_source("256.0.0", "x64", binary)


if __name__ == "__main__":
    unittest.main()
