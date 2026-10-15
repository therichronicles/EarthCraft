#!/usr/bin/env python3
"""Validate version tags and package native, CPU-only EarthCraft releases."""

import argparse
import hashlib
import os
from pathlib import Path
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
    "x86_64-unknown-freebsd": "freebsd-x64",
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


def validate_package_input(version, target, binary):
    if version != workspace_version():
        raise ValueError("Package version must match the workspace manifest")
    if target not in TARGETS:
        raise ValueError(f"Unsupported target: {target}")
    if not binary.is_file():
        raise ValueError(f"Release executable does not exist: {binary}")


def package(version, target, binary, output):
    validate_package_input(version, target, binary)
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
    extension = ".dmg" if "apple" in target else ".zip" if "windows" in target else ".tar.gz"
    archive = output / f"{name}{extension}"
    with tempfile.TemporaryDirectory(prefix="earthcraft-package-") as temporary:
        payload = Path(temporary) / name
        payload.mkdir()
        if "apple" in target:
            import package_tools
            app = package_tools.app_bundle(version, binary.resolve(), payload)
            executable = app / "Contents/MacOS/earthcraft"
        else:
            executable = payload / ("earthcraft.exe" if "windows" in target else "earthcraft")
        if "apple" not in target:
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
                    "background": str(ROOT / "assets" / "packaging" / "macos" / "dmg-background.png"),
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
        if "apple" in target:
            # Reuse the exact signed bundle from the DMG; updater ZIPs keep its
            # full resource tree and executable permissions.
            update_archive = output / f"{name}-update.zip"
            write_update_archive(app, update_archive)
    write_checksum(archive)
    print(archive)


def write_update_archive(app, archive):
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
        stream.write(app, app.name)
        for entry in sorted(app.rglob("*")):
            stream.write(entry, entry.relative_to(app.parent))
    write_checksum(archive)


def write_checksum(archive):
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    # Checksum manifests are consumed on Linux; never translate LF to Windows CRLF.
    archive.with_name(archive.name + ".sha256").write_bytes(
        f"{digest}  {archive.name}\n".encode("utf-8")
    )



def expected_assets(version):
    names = set()
    for target, suffix in TARGETS.items():
        name = f"earthcraft-{version}-{suffix}"
        if "apple" in target:
            names.update((name + ".dmg", name + "-update.zip"))
        elif "windows" in target:
            names.add(name + ".zip")
            names.add(name.removesuffix("-portable") + ".msi")
            if target == "x86_64-pc-windows-msvc":
                names.add(name.removesuffix("-portable") + ".exe")
        else:
            names.add(name + ".tar.gz")
            if "linux" in target:
                names.update(name + "." + extension for extension in
                             ("deb", "rpm", "AppImage", "AppImage.zsync", "flatpak", "snap"))
    return names


def verify_assets(version, output):
    if version != workspace_version():
        raise ValueError("Asset version must match the workspace manifest")
    expected = expected_assets(version)
    expected_files = expected | {name + ".sha256" for name in expected}
    actual = {p.name for p in output.iterdir()}
    if actual != expected_files:
        raise ValueError(f"Release asset mismatch: missing={sorted(expected_files - actual)}, unexpected={sorted(actual - expected_files)}")
    for name in sorted(expected):
        archive = output / name
        with archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        checksum = archive.with_name(name + ".sha256").read_bytes()
        if checksum != f"{digest}  {name}\n".encode("utf-8"):
            raise ValueError(f"Checksum verification failed: {name}")
    print(f"Verified all {len(expected)} release assets and their checksums")


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
    verify = commands.add_parser("verify")
    verify.add_argument("--version", required=True)
    verify.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "validate":
            validate(args.tag)
        elif args.command == "verify":
            verify_assets(args.version, args.output)
        else:
            package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
