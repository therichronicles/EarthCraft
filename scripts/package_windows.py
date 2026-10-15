"""Create Windows MSI installers and the x64 executable beside portable archives."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid
import xml.etree.ElementTree as ET

import package_release as release

ARCHITECTURES = {
    "x86_64-pc-windows-msvc": "x64",
    "aarch64-pc-windows-msvc": "arm64",
}
WIX_NAMESPACE = "http://wixtoolset.org/schemas/v4/wxs"
# MSI upgrades identify the product family independently for each architecture.
UPGRADE_NAMESPACE = uuid.UUID("ff047e82-f6b4-4d1f-ab19-c39c89964371")


def installer_source(version, architecture, binary):
    major, minor, patch = map(int, version.split("-")[0].split("+")[0].split("."))
    if major > 255 or minor > 255 or patch > 65535:
        raise ValueError("MSI versions require major/minor <= 255 and patch <= 65535")
    ET.register_namespace("", WIX_NAMESPACE)

    def element(parent, name, **attributes):
        return ET.SubElement(parent, f"{{{WIX_NAMESPACE}}}{name}", attributes)

    root = ET.Element(f"{{{WIX_NAMESPACE}}}Wix")
    package = element(root, "Package", Name="EarthCraft", Manufacturer="EarthCraft",
                      Version=f"{major}.{minor}.{patch}", Scope="perMachine",
                      UpgradeCode=str(uuid.uuid5(UPGRADE_NAMESPACE, architecture)))
    element(package, "MajorUpgrade", DowngradeErrorMessage="A newer EarthCraft version is already installed.")
    element(package, "MediaTemplate", EmbedCab="yes")
    element(package, "Icon", Id="EarthCraftIcon", SourceFile=str(release.ROOT / "assets/packaging/icon.ico"))
    element(package, "Property", Id="ARPPRODUCTICON", Value="EarthCraftIcon")
    element(package, "Property", Id="ARPHELPLINK", Value="https://github.com/therichronicles/EarthCraft")
    programs = element(package, "StandardDirectory", Id="ProgramFiles6432Folder")
    install = element(programs, "Directory", Id="INSTALLFOLDER", Name="EarthCraft")
    component = element(install, "Component", Id="Application", Guid="*")
    executable = element(component, "File", Id="EarthCraftExe", Source=str(binary.resolve()), KeyPath="yes")
    element(package, "StandardDirectory", Id="ProgramMenuFolder")
    element(executable, "Shortcut", Id="LaunchEarthCraft", Name="EarthCraft", Directory="ProgramMenuFolder",
            Advertise="yes", WorkingDirectory="INSTALLFOLDER", Icon="EarthCraftIcon")
    feature = element(package, "Feature", Id="MainFeature", Title="EarthCraft", Level="1")
    element(feature, "ComponentRef", Id="Application")
    return ET.tostring(root, encoding="unicode", xml_declaration=True)


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Windows installer target: {target}")
    architecture = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    installer = output / f"earthcraft-{version}-windows-{architecture}.msi"
    with tempfile.TemporaryDirectory(prefix="earthcraft-msi-") as temporary:
        source = Path(temporary) / "EarthCraft.wxs"
        source.write_text(installer_source(version, architecture, binary), encoding="utf-8")
        subprocess.run(["wix", "build", "-arch", architecture, "-pdbtype", "none", "-o", str(installer), str(source)], check=True)
    release.write_checksum(installer)
    if architecture == "x64":
        standalone = output / f"earthcraft-{version}-windows-x64.exe"
        shutil.copy2(binary, standalone)
        release.write_checksum(standalone)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Windows packaging failed: {error}\n")


if __name__ == "__main__":
    main()
