"""Offline release-contract and native-tool integration regression checks."""

import hashlib
import json
from pathlib import Path
import plistlib
import subprocess
import tempfile
import tomllib
import unittest
from unittest import mock
import xml.etree.ElementTree as ET

import package_linux as linux
import package_release as release
import package_tools as tools
import package_windows as windows


class NativePackagingTests(unittest.TestCase):
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

    def test_appimage_identity_payload_and_final_zsync_filename(self):
        for target in linux.ARCHITECTURES:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                binary = root / "earthcraft"
                binary.write_bytes(b"native executable fixture")
                archive = root / f"earthcraft-0.1.0-{release.TARGETS[target]}.AppImage"

                def package(config, env):
                    main, executable = config["binaries"]
                    launcher = Path(main["path"])
                    self.assertEqual(launcher.stem, tools.APP_ID)
                    self.assertEqual(Path(executable["path"]).read_bytes(), binary.read_bytes())
                    self.assertIn("EARTHCRAFT_UPDATE_MODE=manual", Path(main["path"]).read_text())
                    desktop = Path(config["deb"]["desktopTemplate"])
                    self.assertIn(f"Exec={launcher.name}\n", desktop.read_text())
                    self.assertEqual(desktop.stem, launcher.stem)
                    self.assertIn(f"Icon={tools.APP_ID}", desktop.read_text())
                    self.assertEqual(config["appimage"]["linuxdeployPlugins"].keys(), {"gtk"})
                    for source, destination in config["appimage"]["files"].items():
                        if destination.endswith(".appdata.xml"):
                            component = ET.parse(source)
                            self.assertEqual(component.findtext("id"), Path(destination).name.removesuffix(".appdata.xml"))
                            self.assertEqual(component.findtext("launchable"), desktop.name)
                    self.assertTrue(env["LDAI_UPDATE_INFORMATION"].endswith(f"{release.TARGETS[target]}.AppImage.zsync"))
                    output = Path(config["outDir"])
                    output.mkdir()
                    (output / "tool-generated.AppImage").write_bytes(b"packaged fixture")

                def zsync(command, **options):
                    self.assertEqual(command[0], "zsyncmake")
                    self.assertEqual(command[command.index("-u") + 1], archive.name)
                    self.assertEqual(Path(command[-1]).read_bytes(), b"packaged fixture")
                    Path(command[command.index("-o") + 1]).write_bytes(b"delta fixture")

                with mock.patch.object(tools, "run", side_effect=package), mock.patch.object(linux.subprocess, "run", side_effect=zsync):
                    linux.native_package("0.1.0", target, binary, archive, root, "appimage")
                delta = archive.with_suffix(".AppImage.zsync")
                self.assertEqual(delta.with_suffix(".zsync.sha256").read_text(),
                                 f"{hashlib.sha256(delta.read_bytes()).hexdigest()}  {delta.name}\n")

    def test_windows_installer_keeps_target_and_portable_binary(self):
        version = release.workspace_version()
        config = tools.configuration("1.2.3-rc.1+build.4", "x86_64-pc-windows-msvc",
                                     Path("earthcraft.exe"), Path("dist"), "wix")
        self.assertEqual(config["version"], "1.2.3")
        for target, architecture in windows.ARCHITECTURES.items():
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                binary = root / "earthcraft.exe"
                binary.write_bytes(b"native Windows fixture")

                def package(config):
                    self.assertEqual(config["targetTriple"], target)
                    self.assertEqual(config["formats"], ["wix"])
                    self.assertFalse(config["windows"]["allowDowngrades"])
                    self.assertEqual(Path(config["binaries"][0]["path"]).with_suffix(".exe"), binary.resolve())
                    (Path(config["outDir"]) / "tool-generated.msi").write_bytes(b"MSI fixture")

                def wix(command, **options):
                    self.assertEqual(command[:4], ["wix", "build", "-arch", "arm64"])
                    self.assertIn(f"Version={version}", command)
                    self.assertIn(f"Binary={binary.resolve()}", command)
                    self.assertIn(f"Icon={release.ROOT / 'assets/packaging/icon.ico'}", command)
                    self.assertTrue(options["check"])
                    manifest = ET.parse(command[-1])
                    namespace = {"w": "http://wixtoolset.org/schemas/v4/wxs"}
                    installer = manifest.find("w:Package", namespace)
                    self.assertEqual(installer.attrib["Scope"], "perMachine")
                    self.assertEqual(installer.attrib["UpgradeCode"], "2dd9ce9e-10dc-56ac-91f3-0f508faf2572")
                    executable = installer.find(".//w:File", namespace)
                    self.assertEqual(executable.attrib["Source"], "$(var.Binary)")
                    shortcut = executable.find("w:Shortcut", namespace)
                    self.assertEqual(shortcut.attrib["Directory"], "ProgramMenuFolder")
                    self.assertEqual(shortcut.attrib["Advertise"], "yes")
                    self.assertEqual(Path(shortcut.attrib["Icon"]).suffix, binary.suffix)
                    self.assertEqual(installer.find("w:Icon", namespace).attrib["Id"], shortcut.attrib["Icon"])
                    component = installer.find(".//w:Component", namespace)
                    reference = installer.find("w:Feature/w:ComponentRef", namespace)
                    self.assertEqual(reference.attrib["Id"], component.attrib["Id"])
                    Path(command[command.index("-o") + 1]).write_bytes(b"MSI fixture")

                with mock.patch.object(tools, "run", side_effect=package) as packager, \
                        mock.patch.object(windows.subprocess, "run", side_effect=wix) as wix_tool:
                    windows.package(version, target, binary, root / "dist")
                self.assertEqual(packager.call_count, int(architecture == "x64"))
                self.assertEqual(wix_tool.call_count, int(architecture == "arm64"))
                installer = root / f"dist/earthcraft-{version}-windows-{architecture}.msi"
                self.assertEqual(installer.read_bytes(), b"MSI fixture")
                standalone = root / f"dist/earthcraft-{version}-windows-x64.exe"
                self.assertEqual(standalone.exists(), architecture == "x64")
                if standalone.exists():
                    self.assertEqual(standalone.read_bytes(), binary.read_bytes())

    def test_mac_bundle_preserves_identity_numeric_version_and_icon(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def package(config):
                self.assertEqual(config["formats"], ["app"])
                self.assertEqual(config["identifier"], tools.APP_ID)
                self.assertEqual(config["macos"]["minimumSystemVersion"], "11.0")
                self.assertEqual(Path(config["icons"][0]).read_bytes(),
                                 (release.ROOT / "assets/packaging/macos/icon.icns").read_bytes())
                with Path(config["macos"]["infoPlistPath"]).open("rb") as stream:
                    plist = plistlib.load(stream)
                self.assertEqual(plist["CFBundleVersion"], "1.2.3")
                self.assertEqual(plist["CFBundleShortVersionString"], "1.2.3")
                self.assertTrue(plist["NSHighResolutionCapable"])
                (Path(config["outDir"]) / "EarthCraft.app").mkdir(parents=True)

            with mock.patch.object(tools, "run", side_effect=package):
                self.assertEqual(tools.app_bundle("1.2.3-rc.1", root / "earthcraft", root / "output"),
                                 root / "output/EarthCraft.app")

    def test_source_build_manifests_keep_cpu_only_locked_builds_and_integration(self):
        flatpak = json.loads((release.ROOT / "packaging/flatpak.json").read_text())
        self.assertEqual(flatpak["app-id"], tools.APP_ID)
        self.assertIn("--env=EARTHCRAFT_UPDATE_MODE=manual", flatpak["finish-args"])
        command = flatpak["modules"][0]["build-commands"][0]
        for argument in ("--locked", "--offline", "--no-default-features"):
            self.assertIn(argument, command)
        snap = json.loads((release.ROOT / "packaging/snapcraft.json").read_text())
        part = snap["parts"]["earthcraft"]
        self.assertEqual(part["plugin"], "rust")
        self.assertEqual(part["rust-channel"], "1.95.0")
        self.assertTrue(part["rust-no-default-features"])
        # Snapcraft's Rust plugin already adds --locked to cargo install.
        self.assertNotIn("--locked", part.get("rust-cargo-parameters", []))
        self.assertEqual(snap["apps"]["earthcraft"]["environment"]["EARTHCRAFT_UPDATE_MODE"], "manual")
        self.assertIn("craftctl default", part["override-build"])
        with (release.ROOT / "apps/earthcraft/Cargo.toml").open("rb") as stream:
            rpm = tomllib.load(stream)["package"]["metadata"]["generate-rpm"]
        self.assertEqual(rpm["requires"]["gtk3"], "*")
        for asset in rpm["assets"][1:]:
            self.assertTrue((release.ROOT / asset["source"]).is_file())

    def test_snapcraft_receives_the_release_version_instead_of_a_template_version(self):
        template = json.loads((release.ROOT / "packaging/snapcraft.json").read_text())
        self.assertNotIn("version", template)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "earthcraft.snap"

            def snapcraft(command, **options):
                if command[:2] == ["sudo", "chown"]:
                    self.assertIn(f"--reference={root / 'source'}", command)
                    self.assertIn(str(archive), command)
                    return
                config = json.loads((options["cwd"] / "snap/snapcraft.yaml").read_text())
                self.assertEqual(config["version"], "7.8.9")
                self.assertEqual(config["architectures"][0]["build-for"], ["arm64"])
                archive.write_bytes(b"Snap fixture")

            with mock.patch.object(linux, "source_snapshot", return_value=root / "source"), mock.patch.object(linux.subprocess, "run", side_effect=snapcraft):
                linux.snap("7.8.9", "arm64", archive, root)

    def test_snap_build_failure_restores_ownership_and_preserves_the_build_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            commands = []

            def snapcraft(command, **options):
                commands.append(command)
                if "snapcraft" in command:
                    raise subprocess.CalledProcessError(9, command)
                self.assertEqual(command, ["sudo", "chown", "-R", f"--reference={source}",
                                           str(root / "snap-project")])

            with mock.patch.object(linux, "source_snapshot", return_value=source), mock.patch.object(linux.subprocess, "run", side_effect=snapcraft):
                with self.assertRaises(subprocess.CalledProcessError) as error:
                    linux.snap("7.8.9", "amd64", root / "earthcraft.snap", root)
            self.assertEqual(error.exception.returncode, 9)
            self.assertEqual(len(commands), 2)

    def test_flatpak_uses_maintained_builder_and_retains_appstream_composition(self):
        for architecture in ("x86_64", "aarch64"):
            with self.subTest(architecture=architecture), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source = root / "source"
                source.mkdir()
                archive = root / "earthcraft.flatpak"
                commands = []

                def flatpak(command, **options):
                    commands.append(command)
                    if command[:5] == ["dbus-run-session", "--", "flatpak", "run", "org.flatpak.Builder"]:
                        self.assertIn("--system", command)
                        self.assertIn(f"--arch={architecture}", command)
                        self.assertIn(f"--state-dir={root / 'flatpak-state'}", command)
                        manifest = json.loads(Path(command[-1]).read_text())
                        self.assertTrue(manifest.get("appstream-compose", True))
                        self.assertEqual(manifest["modules"][0]["sources"], [{"type": "dir", "path": str(source)}])
                    elif command[:2] == ["flatpak", "build-bundle"]:
                        archive.write_bytes(b"Flatpak fixture")

                with mock.patch.object(linux, "source_snapshot", return_value=source), mock.patch.object(linux.subprocess, "run", side_effect=flatpak):
                    linux.flatpak(archive, architecture, root)
                self.assertEqual(len(commands), 3)
                self.assertEqual(archive.read_bytes(), b"Flatpak fixture")


if __name__ == "__main__":
    unittest.main()
