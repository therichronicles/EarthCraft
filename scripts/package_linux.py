"""Package native Linux binaries as DEB, RPM, AppImage, Flatpak and Snap files."""

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import package_release as release

APP_ID = "com.theri.earthcraft"
FLATPAK_RUNTIME_VERSION = "49"


@dataclass(frozen=True)
class Architecture:
    package: str
    desktop: str


ARCHITECTURES = {
    "x86_64-unknown-linux-gnu": Architecture("amd64", "x86_64"),
    "aarch64-unknown-linux-gnu": Architecture("arm64", "aarch64"),
}
FORMATS = ("deb", "rpm", "appimage", "flatpak", "snap")


def stage_application(directory, binary):
    files = {
        "usr/bin/earthcraft": binary,
        "usr/share/applications/earthcraft.desktop": release.ROOT / "packaging/earthcraft.desktop",
        "usr/share/icons/hicolor/256x256/apps/earthcraft.png": release.ROOT / "assets/icon_256.png",
        "usr/share/licenses/earthcraft/LICENSE": release.ROOT / "LICENSE",
        f"usr/share/metainfo/{APP_ID}.metainfo.xml": release.ROOT / "packaging/earthcraft.metainfo.xml",
    }
    for destination, source in files.items():
        path = directory / destination
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, path)
        path.chmod(0o755 if destination == "usr/bin/earthcraft" else 0o644)
    return files


def nfpm_configuration(version, architecture, directory, files):
    return {
        "name": "earthcraft", "arch": architecture, "platform": "linux",
        "version": version, "section": "science", "priority": "optional",
        "maintainer": "EarthCraft <earthcraft@localhost>", "license": "MIT",
        "homepage": "https://github.com/therichronicles/EarthCraft",
        "description": "3D globe explorer with satellite imagery, live layers and KML places",
        "contents": [{"src": str(directory / name), "dst": "/" + name} for name in files],
        "overrides": {
            "deb": {"depends": ["libc6 (>= 2.35)", "libgcc-s1", "libgtk-3-0", "libx11-6", "libxkbcommon0", "libwayland-client0", "xdg-utils"], "recommends": ["espeak"]},
            "rpm": {"depends": ["glibc >= 2.35", "libgcc", "gtk3", "libX11", "libxkbcommon", "wayland-libs", "xdg-utils"], "recommends": ["espeak-ng"]},
        },
    }


def appimage(directory, archive, architecture, source):
    # linuxdeploy collects linked libraries and its GTK plugin collects schemas,
    # loaders and modules; shipping just the executable would omit native dialogs.
    env = {**os.environ, "ARCH": architecture, "APPIMAGE_EXTRACT_AND_RUN": "1",
           "LDAI_OUTPUT": str(archive),
           "LDAI_UPDATE_INFORMATION": f"gh-releases-zsync|therichronicles|EarthCraft|latest|earthcraft-*-linux-{'x64' if architecture == 'x86_64' else 'arm64'}.AppImage.zsync"}
    subprocess.run(["linuxdeploy", "--appdir", str(directory),
                    "--executable", str(source / "usr/bin/earthcraft"),
                    "--desktop-file", str(source / "usr/share/applications/earthcraft.desktop"),
                    "--icon-file", str(source / "usr/share/icons/hicolor/256x256/apps/earthcraft.png"),
                    "--plugin", "gtk"], check=True, env=env)
    # Preserve GTK's generated setup hooks while keeping managed bundle updates
    # outside the portable executable replacement path.
    launcher = directory / "AppRun"
    if launcher.is_symlink():
        launcher.unlink()
    launcher.write_text('''#!/bin/sh
set -e
APPDIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export APPDIR EARTHCRAFT_UPDATE_MODE=manual
export LD_LIBRARY_PATH="$APPDIR/usr/lib:${LD_LIBRARY_PATH:-}"
export PATH="$APPDIR/usr/bin:$PATH"
for hook in "$APPDIR"/apprun-hooks/*.sh; do
    if [ -r "$hook" ]; then . "$hook"; fi
done
exec "$APPDIR/usr/bin/earthcraft" "$@"
''', encoding="utf-8")
    launcher.chmod(0o755)
    # appimagetool derives this legacy filename from earthcraft.desktop.
    # Rename only the isolated AppDir copy; other formats retain the app ID.
    metadata = directory / "usr/share/metainfo" / f"{APP_ID}.metainfo.xml"
    metadata.rename(metadata.with_name("earthcraft.appdata.xml"))
    # zsyncmake writes its sidecar in the working directory, even when the
    # AppImage destination is absolute. Keep both release files together.
    subprocess.run(["linuxdeploy", "--appdir", str(directory), "--output", "appimage"],
                   check=True, env=env, cwd=archive.parent)
    delta = archive.with_suffix(archive.suffix + ".zsync")
    if not delta.is_file():
        raise ValueError("AppImage packaging did not create its zsync update metadata")
    release.write_checksum(delta)


def flatpak(directory, archive, architecture, temporary):
    build = temporary / "flatpak-build"
    repository = temporary / "flatpak-repository"
    subprocess.run(["flatpak", "build-init", f"--arch={architecture}", str(build), APP_ID,
                    "org.gnome.Platform", "org.gnome.Platform", FLATPAK_RUNTIME_VERSION], check=True)
    shutil.copytree(directory / "usr", build / "files", dirs_exist_ok=True)
    files = build / "files"
    desktop = files / "share/applications/earthcraft.desktop"
    (desktop.parent / f"{APP_ID}.desktop").write_text(
        desktop.read_text().replace("Icon=earthcraft", f"Icon={APP_ID}"), encoding="utf-8")
    desktop.unlink()
    metadata = files / f"share/metainfo/{APP_ID}.metainfo.xml"
    metadata.write_text(metadata.read_text().replace("earthcraft.desktop", f"{APP_ID}.desktop"), encoding="utf-8")
    (files / "share/icons/hicolor/256x256/apps/earthcraft.png").rename(
        files / f"share/icons/hicolor/256x256/apps/{APP_ID}.png")
    subprocess.run(["flatpak", "build-finish", "--command=earthcraft", "--share=network", "--share=ipc",
                    "--socket=wayland", "--socket=fallback-x11", "--device=dri", "--filesystem=host",
                    "--env=EARTHCRAFT_UPDATE_MODE=manual", str(build)], check=True)
    subprocess.run(["flatpak", "build-export", str(repository), str(build), "stable"], check=True)
    subprocess.run(["flatpak", "build-bundle", f"--arch={architecture}",
                    "--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo",
                    str(repository), str(archive), APP_ID, "stable"], check=True)


def snap_configuration(version, architecture, directory):
    return {
        "name": "earthcraft", "base": "core22", "version": version,
        "summary": "Explore the Earth in a 3D globe",
        "description": "Satellite imagery, live layers, search, KML places and distance measurements.",
        "grade": "stable", "confinement": "strict", "license": "MIT",
        "architectures": [{"build-on": [architecture], "build-for": [architecture]}],
        "apps": {"earthcraft": {"command": "usr/bin/earthcraft",
            "desktop": "usr/share/applications/earthcraft.desktop",
            "extensions": ["gnome"],
            "plugs": ["network", "home", "removable-media", "audio-playback"],
            "environment": {"EARTHCRAFT_UPDATE_MODE": "manual"}}},
        "parts": {"earthcraft": {"plugin": "dump", "source": str(directory),
            "stage-packages": ["libgtk-3-0", "libxkbcommon0", "libx11-6", "libwayland-client0", "xdg-utils", "espeak"]}},
    }


def package(version, target, binary, output, formats):
    release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux package target: {target}")
    architecture = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = f"earthcraft-{version}-{release.TARGETS[target]}"
    # Snapcraft runs in a snap mount namespace with a private /tmp. Keep its
    # project in the ignored build tree so it can access the staged source.
    workspace = release.ROOT / "target/packaging"
    workspace.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="earthcraft-linux-", dir=workspace) as temporary_name:
        temporary = Path(temporary_name)
        directory = temporary / "application"
        files = stage_application(directory, binary)
        configuration = temporary / "nfpm.yaml"
        # JSON is a YAML subset and avoids requiring a Python YAML dependency.
        configuration.write_text(json.dumps(nfpm_configuration(version, architecture.package, directory, files)), encoding="utf-8")
        for format_name in formats:
            extension = "AppImage" if format_name == "appimage" else format_name
            archive = output / f"{name}.{extension}"
            if format_name in ("deb", "rpm"):
                subprocess.run(["nfpm", "package", "--config", str(configuration), "--packager", format_name, "--target", str(archive)], check=True)
            elif format_name == "appimage":
                # linuxdeploy mutates its AppDir, so isolate it from other formats.
                appdir = temporary / "AppDir"
                shutil.copytree(directory, appdir)
                appimage(appdir, archive, architecture.desktop, directory)
            elif format_name == "flatpak":
                flatpak(directory, archive, architecture.desktop, temporary)
            elif format_name == "snap":
                project = temporary / "snap-project"
                (project / "snap").mkdir(parents=True)
                (project / "snap/snapcraft.yaml").write_text(json.dumps(
                    snap_configuration(version, architecture.package, directory)), encoding="utf-8")
                subprocess.run(["snapcraft", "--destructive-mode", "--output", str(archive)], cwd=project, check=True)
            else:
                raise ValueError(f"Unsupported Linux format: {format_name}")
            if not archive.is_file():
                raise ValueError(f"Packaging did not create {archive.name}")
            release.write_checksum(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--formats", nargs="+", choices=FORMATS, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output, args.formats)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Linux packaging failed: {error}\n")


if __name__ == "__main__":
    main()
