"""Offline archive, metadata and release-version regression checks."""

import hashlib
import os
from pathlib import Path
import tarfile
import tempfile
import struct
import unittest
import zipfile

import package_release as release


class ReleasePackagingTests(unittest.TestCase):
    def test_archive_contents_permissions_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            binary.write_bytes(b"archive fixture, not a native executable")
            version = release.workspace_version()
            for target in release.TARGETS:
                if "apple" in target:
                    continue  # Native Mach-O/signing is verified on macOS separately.
                with self.subTest(target=target):
                    output = root / target
                    release.package(version, target, binary, output)
                    archive = next(p for p in output.iterdir() if p.suffix != ".sha256")
                    name = f"earthcraft-{version}-{release.TARGETS[target]}"
                    self.assertEqual(
                        archive.name,
                        name + (".zip" if "windows" in target else ".tar.gz"),
                    )
                    if "windows" not in target:
                        with tarfile.open(archive) as contents:
                            self.assertEqual(contents.getnames(), ["earthcraft"])
                            executable = contents.getmember("earthcraft")
                            if os.name != "nt":
                                self.assertEqual(executable.mode & 0o777, 0o755)
                            self.assertEqual(contents.extractfile(executable).read(), binary.read_bytes())
                    else:
                        with zipfile.ZipFile(archive) as contents:
                            self.assertEqual(contents.namelist(), ["earthcraft.exe"])
                            self.assertEqual(contents.read("earthcraft.exe"), binary.read_bytes())
                    expected = hashlib.sha256(archive.read_bytes()).hexdigest()
                    self.assertEqual(
                        archive.with_name(archive.name + ".sha256").read_bytes(),
                        f"{expected}  {archive.name}\n".encode("utf-8"),
                    )

    def test_updater_zip_keeps_whole_bundle_bytes_and_executable_mode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "EarthCraft.app"
            executable = app / "Contents" / "MacOS" / "earthcraft"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"signed executable fixture")
            executable.chmod(0o755)
            signature = app / "Contents" / "_CodeSignature" / "CodeResources"
            signature.parent.mkdir()
            signature.write_bytes(b"bundle signature fixture")
            archive = root / "earthcraft-0.1.0-macos-arm64-update.zip"
            release.write_update_archive(app, archive)
            with zipfile.ZipFile(archive) as contents:
                for entry in (executable, signature):
                    self.assertEqual(contents.read(entry.relative_to(root).as_posix()), entry.read_bytes())
                if os.name != "nt":
                    info = contents.getinfo("EarthCraft.app/Contents/MacOS/earthcraft")
                    self.assertEqual((info.external_attr >> 16) & 0o777, 0o755)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            self.assertEqual(archive.with_name(archive.name + ".sha256").read_bytes(),
                             f"{digest}  {archive.name}\n".encode("utf-8"))

    def test_macos_rejects_invalid_architecture_headers(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            for header in [
                b"invalid",
                struct.pack("<II", 0xFEEDFACF, 0x01000007),
                bytes.fromhex("ca fe ba be 00 00 00 02"),
            ]:
                binary.write_bytes(header)
                with self.subTest(header=header), self.assertRaisesRegex(
                    ValueError, "ARM64 Mach-O"
                ):
                    release.package(
                        release.workspace_version(), "aarch64-apple-darwin", binary, root / "out"
                    )

    def test_version_and_binary_validation(self):
        with self.assertRaises(ValueError):
            release.validate("v9999.0.0")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            with self.assertRaisesRegex(ValueError, "Release executable does not exist"):
                release.package(release.workspace_version(), next(iter(release.TARGETS)), binary, root / "out")


if __name__ == "__main__":
    unittest.main()
