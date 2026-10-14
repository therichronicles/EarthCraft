#!/usr/bin/env python3
"""Validate version tags and package native, CPU-only EarthCraft releases."""

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import struct
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-gnu": "linux-x64",
    "aarch64-unknown-linux-gnu": "linux-arm64",
    "x86_64-pc-windows-msvc": "windows-x64-portable",
    "aarch64-pc-windows-msvc": "windows-arm64-portable",
    "aarch64-apple-darwin": "macos-arm64",
}
VERSION = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def workspace_version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["workspace"]["package"]["version"]
    if not VERSION.fullmatch(version):
        raise ValueError(f"Unsupported workspace version: {version}")
    return version


def validate(tag):
    version = workspace_version()
    if tag and tag != f"v{version}":
        raise ValueError(f"Tag {tag!r} must match workspace version v{version}")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as stream:
            stream.write(f"version={version}\n")
    print(version)


def package(version, target, binary, output):
    if version != workspace_version():
        raise ValueError("Package version must match the workspace manifest")
    if target not in TARGETS:
        raise ValueError(f"Unsupported target: {target}")
    if not binary.is_file():
        raise ValueError(f"Release executable does not exist: {binary}")
    if target == "aarch64-apple-darwin":
        # Require a thin ARM64 Mach-O executable.
        mach_o_64_magic = 0xFEEDFACF
        cpu_type_arm64 = 0x0100000C
        with binary.open("rb") as stream:
            header = stream.read(8)
        if header != struct.pack("<II", mach_o_64_magic, cpu_type_arm64):
            raise ValueError("macOS release executable must be an ARM64 Mach-O binary")
    output.mkdir(parents=True, exist_ok=True)
    name = f"earthcraft-{version}-{TARGETS[target]}"
    extension = ".dmg" if "apple" in target else ".tar.gz" if "linux" in target else ".zip"
    archive = output / f"{name}{extension}"
    with tempfile.TemporaryDirectory(prefix="earthcraft-package-") as temporary:
        payload = Path(temporary) / name
        payload.mkdir()
        if "apple" in target:
            contents = payload / "EarthCraft.app" / "Contents"
            executable = contents / "MacOS" / "earthcraft"
            executable.parent.mkdir(parents=True)
            resources = contents / "Resources"
            resources.mkdir()
            shutil.copy2(ROOT / "assets" / "macos" / "icon.icns", resources / "EarthCraft.icns")
            with (contents / "Info.plist").open("wb") as stream:
                plistlib.dump(
                    {
                        "CFBundleIdentifier": "com.theri.earthcraft",
                        "CFBundleName": "EarthCraft",
                        "CFBundleDisplayName": "EarthCraft",
                        "CFBundleExecutable": "earthcraft",
                        "CFBundleIconFile": "EarthCraft.icns",
                        "CFBundlePackageType": "APPL",
                        "CFBundleVersion": version.split("-")[0].split("+")[0],
                        "CFBundleShortVersionString": version.split("-")[0].split("+")[0],
                        "LSMinimumSystemVersion": "11.0",
                        "NSHighResolutionCapable": True,
                    },
                    stream,
                )
        else:
            executable = payload / ("earthcraft.exe" if "windows" in target else "earthcraft")
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        if "apple" in target:
            app = payload / "EarthCraft.app"
            subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
            subprocess.run(
                ["codesign", "--verify", "--deep", "--strict", str(app)], check=True
            )
        if extension == ".dmg":
            try:
                import dmgbuild
            except ImportError as error:
                raise ValueError(
                    "macOS packaging requires dmgbuild; install with "
                    "python -m pip install dmgbuild==1.6.7"
                ) from error
            # Hiding .app extensions writes FinderInfo onto the signed bundle,
            # which fails strict signature verification. Leave its metadata intact.
            dmgbuild.build_dmg(
                str(archive),
                "EarthCraft",
                settings={
                    "format": "UDZO",
                    "files": [str(app)],
                    "symlinks": {"Applications": "/Applications"},
                    "background": str(ROOT / "assets" / "macos" / "dmg-background.png"),
                    "window_rect": ((200, 200), (640, 270)),
                    "default_view": "icon-view",
                    "icon_size": 128,
                    "text_size": 14,
                    # Finder adds padding inside each icon cell. The background
                    # arrow aligns with the displayed centers, including that inset.
                    "icon_locations": {
                        "EarthCraft.app": (150, 70),
                        "Applications": (490, 70),
                    },
                },
            )
            subprocess.run(["hdiutil", "verify", str(archive)], check=True)
        elif extension == ".tar.gz":
            with tarfile.open(archive, "w:gz") as stream:
                stream.add(executable, arcname=executable.name)
        else:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
                stream.write(executable, executable.name)
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    # Checksum manifests are consumed on Linux; never translate LF to Windows CRLF.
    archive.with_name(archive.name + ".sha256").write_bytes(
        f"{digest}  {archive.name}\n".encode("utf-8")
    )
    print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    version = commands.add_parser("validate")
    version.add_argument("--tag", default="")
    archive = commands.add_parser("package")
    archive.add_argument("--version", required=True)
    archive.add_argument("--target", choices=TARGETS, required=True)
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "validate":
            validate(args.tag)
        else:
            package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
