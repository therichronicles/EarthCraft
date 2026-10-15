"""Shared configuration and invocation of cargo-packager for native packages."""

import json
from pathlib import Path
import plistlib
import subprocess

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "com.theri.earthcraft"


def configuration(version, target, binary, output, format_name):
    config = json.loads((ROOT / "packaging/cargo-packager.json").read_text())
    executable = binary.resolve()
    if "windows" in target:
        executable = executable.with_suffix("")
    if format_name == "wix":
        # MSI uses numeric versions; release filenames retain the full semver.
        version = version.split("-")[0].split("+")[0]
    config.update(version=version, targetTriple=target, outDir=str(output.resolve()),
                  binaries=[{"path": str(executable), "main": True}], formats=[format_name])
    icon = "macos/icon.icns" if "apple" in target else "icon.ico" if "windows" in target else "icon_256.png"
    config["icons"] = [str(ROOT / "assets/packaging" / icon)]
    return config


def run(config, env=None):
    output = Path(config["outDir"])
    output.mkdir(parents=True, exist_ok=True)
    # Raw JSON avoids changing the working directory to a temporary config file.
    subprocess.run(["cargo", "packager", "--config", json.dumps(config)],
                   cwd=ROOT, env=env, check=True)


def artifact(output, extension):
    paths = list(output.glob(f"*.{extension}"))
    if len(paths) != 1:
        raise ValueError(f"Expected one {extension} package, found {len(paths)} in {output}")
    return paths[0]


def app_bundle(version, binary, output):
    config = configuration(version, "aarch64-apple-darwin", binary, output, "app")
    # Override the packager's timestamp build number with our numeric release
    # version, preserving the existing bundle metadata and updater contract.
    plist_path = output.parent / "Info.plist"
    numeric_version = version.split("-")[0].split("+")[0]
    with plist_path.open("wb") as stream:
        plistlib.dump({"CFBundleVersion": numeric_version,
                      "CFBundleShortVersionString": numeric_version,
                      "NSHighResolutionCapable": True}, stream)
    config["macos"]["infoPlistPath"] = str(plist_path)
    run(config)
    return artifact(output, "app")
