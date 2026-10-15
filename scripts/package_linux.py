"""Coordinate native Linux packaging tools and preserve release asset names."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

import package_release as release
import package_tools as tools

APP_ID = tools.APP_ID
ARCHITECTURES = {
    "x86_64-unknown-linux-gnu": ("amd64", "x86_64"),
    "aarch64-unknown-linux-gnu": ("arm64", "aarch64"),
}
FORMATS = ("deb", "rpm", "appimage", "flatpak", "snap")


def desktop_files():
    return {
        str(release.ROOT / "LICENSE"): "usr/share/licenses/earthcraft/LICENSE",
        str(release.ROOT / "packaging/earthcraft.metainfo.xml"): f"usr/share/metainfo/{APP_ID}.metainfo.xml",
    }


def native_package(version, target, binary, archive, temporary, format_name):
    output = temporary / format_name
    config = tools.configuration(version, target, binary, output, format_name)
    config["deb"].update(desktopTemplate=str(release.ROOT / "packaging/earthcraft.desktop"),
                         files=desktop_files())
    env = dict(os.environ)
    if format_name == "appimage":
        # cargo-packager derives desktop/icon names from the main file's stem.
        # An extension keeps the full dotted ID instead of dropping .earthcraft.
        launcher = temporary / f"{APP_ID}.sh"
        shutil.copy2(release.ROOT / "packaging/appimage-launcher.sh", launcher)
        launcher.chmod(0o755)
        config["binaries"] = [{"path": str(launcher), "main": True},
                              {"path": str(binary.resolve()), "main": False}]
        desktop = temporary / f"{APP_ID}.desktop"
        desktop.write_text((release.ROOT / "packaging/earthcraft.desktop").read_text()
                           .replace("Exec=earthcraft", f"Exec={launcher.name}")
                           .replace("Icon=earthcraft", f"Icon={APP_ID}"), encoding="utf-8")
        metadata = temporary / f"{APP_ID}.appdata.xml"
        metadata.write_text((release.ROOT / "packaging/earthcraft.metainfo.xml").read_text()
                            .replace("earthcraft.desktop", desktop.name), encoding="utf-8")
        config["deb"]["desktopTemplate"] = str(desktop)
        config["appimage"]["files"] = {
            str(metadata): f"usr/share/metainfo/{metadata.name}",
            str(release.ROOT / "LICENSE"): "usr/share/licenses/earthcraft/LICENSE",
        }
        env.update(APPIMAGE_EXTRACT_AND_RUN="1",
                   LDAI_UPDATE_INFORMATION=f"gh-releases-zsync|therichronicles|EarthCraft|latest|earthcraft-*-{release.TARGETS[target]}.AppImage.zsync")
    tools.run(config, env)
    extension = "AppImage" if format_name == "appimage" else "deb"
    shutil.copy2(tools.artifact(output, extension), archive)
    if format_name == "appimage":
        # Generate against the final filename so Filename/URL headers and release
        # links agree, independently of cargo-packager's internal output names.
        delta = archive.with_suffix(".AppImage.zsync")
        subprocess.run(["zsyncmake", "-o", str(delta), "-f", archive.name,
                        "-u", archive.name, str(archive)], check=True)
        release.write_checksum(delta)


def rpm(version, target, binary, archive):
    # Keep package metadata in Cargo.toml, overriding only the supplied binary.
    with (release.ROOT / "apps/earthcraft/Cargo.toml").open("rb") as stream:
        assets = tomllib.load(stream)["package"]["metadata"]["generate-rpm"]["assets"]
    assets[0]["source"] = str(binary.resolve())
    # JSON strings/inline tables form valid TOML values here.
    entries = ["{ " + ", ".join(f"{key} = {json.dumps(value)}" for key, value in entry.items()) + " }"
               for entry in assets]
    override = "assets = [" + ", ".join(entries) + "]"
    subprocess.run(["cargo", "generate-rpm", "-p", "apps/earthcraft", "--target", target,
                    "--output", str(archive), "--set-metadata", override], cwd=release.ROOT, check=True)


def source_snapshot(temporary):
    # Build the reviewed checkout without local caches, settings or target output.
    archive = temporary / "source.tar"
    subprocess.run(["git", "archive", "--format=tar", "--output", str(archive), "HEAD"],
                   cwd=release.ROOT, check=True)
    source = temporary / "source"
    source.mkdir()
    with tarfile.open(archive) as stream:
        stream.extractall(source, filter="data")
    return source


def flatpak(archive, architecture, temporary):
    source = source_snapshot(temporary)
    # flatpak-builder's build sandbox has no network. Resolve locked crates on
    # the host first and supply Cargo's generated offline source configuration.
    (source / ".cargo").mkdir(exist_ok=True)
    with (source / ".cargo/config.toml").open("w", encoding="utf-8") as config:
        subprocess.run(["cargo", "vendor", "--locked", "--versioned-dirs", "vendor"],
                       cwd=source, check=True, stdout=config)
    manifest = json.loads((release.ROOT / "packaging/flatpak.json").read_text())
    manifest["modules"][0]["sources"] = [{"type": "dir", "path": str(source)}]
    manifest_path = temporary / "flatpak.json"
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    repository = temporary / "flatpak-repository"
    # The maintained builder supports GNOME 49's appstreamcli compose command;
    # Ubuntu 22.04's native builder still requires the removed appstream-compose.
    # The sandboxed builder invokes host Flatpak through a D-Bus session.
    subprocess.run(["dbus-run-session", "--", "flatpak", "run", "org.flatpak.Builder",
                    "--system", "--force-clean",
                    f"--state-dir={temporary / 'flatpak-state'}", f"--arch={architecture}",
                    f"--repo={repository}", str(temporary / "flatpak-build"), str(manifest_path)], check=True)
    subprocess.run(["flatpak", "build-bundle", f"--arch={architecture}",
                    "--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo",
                    str(repository), str(archive), APP_ID, "stable"], check=True)


def snap(version, architecture, archive, temporary):
    source = source_snapshot(temporary)
    config = json.loads((release.ROOT / "packaging/snapcraft.json").read_text())
    config["version"] = version
    config["architectures"] = [{"build-on": [architecture], "build-for": [architecture]}]
    config["parts"]["earthcraft"]["source"] = str(source)
    project = temporary / "snap-project"
    (project / "snap").mkdir(parents=True)
    # JSON is valid YAML; no additional Python dependency is needed.
    (project / "snap/snapcraft.yaml").write_text(json.dumps(config), encoding="utf-8")
    try:
        subprocess.run(["sudo", "env", f"PATH={os.environ['PATH']}", "snapcraft",
                        "--destructive-mode", "--output", str(archive)], cwd=project, check=True)
    finally:
        # Snapcraft builds as root. Restore the caller's ownership on both
        # success and failure before TemporaryDirectory removes the build tree.
        owned_paths = [str(project)]
        if archive.exists():
            owned_paths.append(str(archive))
        subprocess.run(["sudo", "chown", "-R", f"--reference={source}", *owned_paths], check=True)


def package(version, target, binary, output, formats):
    if version != release.workspace_version():
        raise ValueError("Package version must match the workspace manifest")
    if any(name in ("deb", "rpm", "appimage") for name in formats):
        if binary is None:
            raise ValueError("DEB, RPM and AppImage packaging require --binary")
        release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux package target: {target}")
    package_arch, desktop_arch = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = f"earthcraft-{version}-{release.TARGETS[target]}"
    # Snapcraft has a private /tmp namespace; keep its project in the build tree.
    workspace = release.ROOT / "target/packaging"
    workspace.mkdir(parents=True, exist_ok=True)
    for format_name in formats:
        with tempfile.TemporaryDirectory(prefix=f"earthcraft-{format_name}-", dir=workspace) as temporary_name:
            temporary = Path(temporary_name)
            extension = "AppImage" if format_name == "appimage" else format_name
            archive = output / f"{name}.{extension}"
            if format_name in ("deb", "appimage"):
                native_package(version, target, binary, archive, temporary, format_name)
            elif format_name == "rpm":
                rpm(version, target, binary, archive)
            elif format_name == "flatpak":
                flatpak(archive, desktop_arch, temporary)
            elif format_name == "snap":
                snap(version, package_arch, archive, temporary)
            else:
                raise ValueError(f"Unsupported Linux format: {format_name}")
            if not archive.is_file():
                raise ValueError(f"Packaging did not create {archive.name}")
            release.write_checksum(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--formats", nargs="+", choices=FORMATS, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output, args.formats)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Linux packaging failed: {error}\n")


if __name__ == "__main__":
    main()
