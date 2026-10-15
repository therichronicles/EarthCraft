"""Create Windows MSI installers and the x64 executable beside portable archives."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile

import package_release as release
import package_tools as tools

ARCHITECTURES = {
    "x86_64-pc-windows-msvc": "x64",
    "aarch64-pc-windows-msvc": "arm64",
}


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Windows installer target: {target}")
    architecture = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    installer = output / f"earthcraft-{version}-windows-{architecture}.msi"
    with tempfile.TemporaryDirectory(prefix="earthcraft-msi-") as temporary:
        output_directory = Path(temporary)
        config = tools.configuration(version, target, binary, output_directory, "wix")
        tools.run(config)
        shutil.copy2(tools.artifact(output_directory, "msi"), installer)
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
