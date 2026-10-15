# Releases

The [release workflow](../.github/workflows/release.yml) builds locked release
binaries with Rust 1.95 (Flatpak builds use its Rust SDK extension). Portable archives contain the application executable;
native installers and Linux bundles also contain their desktop integration and
runtime/package metadata. macOS keeps its Apple Silicon DMG and a ZIP of that
same signed app bundle for in-app updates.
These artifacts use `--no-default-features`: experimental GPU globe rendering is excluded
from release downloads on every platform. CPU rendering remains the default.

Each native build restores Cargo dependency caches through `Swatinem/rust-cache@v2`
after installing Rust. Release cache keys include the target architecture and stay
separate from development checks. An initial run still compiles dependencies;
compatible later runs can reuse them. Caching does not reuse previously published
application binaries: every release still builds and packages its tagged source.

| Platform | Release files (prefix `earthcraft-0.1.0-`) |
| --- | --- |
| Linux x86-64 | `linux-x64.tar.gz`, `linux-x64.deb`, `linux-x64.rpm`, `linux-x64.AppImage`, `linux-x64.AppImage.zsync`, `linux-x64.flatpak`, `linux-x64.snap` |
| Linux ARM64 | `linux-arm64.tar.gz`, `linux-arm64.deb`, `linux-arm64.rpm`, `linux-arm64.AppImage`, `linux-arm64.AppImage.zsync`, `linux-arm64.flatpak`, `linux-arm64.snap` |
| Windows x86-64 | `windows-x64-portable.zip`, `windows-x64.msi`, `windows-x64.exe` |
| Windows ARM64 | `windows-arm64-portable.zip`, `windows-arm64.msi` |
| FreeBSD x86-64 | `freebsd-x64.tar.gz` |
| macOS Apple Silicon (ARM64) | `macos-arm64.dmg` containing `EarthCraft.app`; `macos-arm64-update.zip` for the updater |

One `SHA256SUMS.txt` lists all 22 release assets, including AppImage zsync metadata.
Publication first verifies the exact asset set and every checksum; a missing format
or unexpected build file prevents publication. Preview artifacts retain
individual `.sha256` files; publication verifies and consolidates them.
GitHub also provides its standard source-code archives. Linux builds run
on Ubuntu 22.04, targeting glibc 2.35 or newer. Portable Linux archives require
GTK 3 and a working graphics driver. AppImage bundles include GTK libraries and
resources through linuxdeploy's GTK plugin. FreeBSD builds run in a FreeBSD 14.5
VM and require GTK 3, libxkbcommon and a working graphics driver at runtime.
macOS bundles target macOS 11 or newer and are ad-hoc signed, without a
Developer ID certificate or notarization. The ARM64 build packages `EarthCraft.app`
directly into a DMG. Packaging verifies its ARM64 Mach-O header, embeds the padded
macOS icon, then signs and verifies the assembled app. Ad-hoc signing does not
establish a trusted publisher or remove Gatekeeper prompts.
Windows/Linux/FreeBSD portable archives contain just the executable at their root. On macOS,
open the DMG and drag EarthCraft along the arrow onto the Applications shortcut.
The window opens in icon view with the app on the left and Applications on the
right. Only those two items are visible; Finder layout/background metadata stays
hidden. There are no bundled
READMEs, agent rules, contributor docs or changelogs in portable downloads.
Linux installer bundles include the MIT license. Build/package checks do not verify
interactive runtime behavior: manually test downloads on the supported platforms
before announcing them.


## Installing native packages

DEB and RPM packages install the app, desktop entry and icon through the system
package manager, with GTK and other runtime dependencies declared. MSI packages
install to Program Files and add a Start menu shortcut; they require administrator
access. cargo-packager manages x64 MSI metadata through its WiX backend. ARM64
uses WiX 5 with `packaging/windows-arm64.wxs`, retaining a stable upgrade code,
because cargo-packager 0.11.8 bundles WiX 3.11 without ARM64 support. Both use
the numeric major/minor/patch version within Windows Installer limits. The x64
standalone EXE is the same application binary as the portable ZIP.
Optional narration requires `espeak` on DEB installations; RPM recommends
`espeak-ng`, and the Snap includes `espeak`.

AppImage files are executable bundles; run `chmod +x earthcraft-*-linux-*.AppImage`
and launch the file for your architecture. Matching `.zsync` files provide
AppImage delta-update metadata. AppImage, Flatpak and Snap launchers select manual
updates so the in-app updater cannot replace a bundled executable. MSI and
system-package paths use the updater's existing managed-install detection.

Install a Flatpak bundle with `flatpak install --user ./earthcraft-<version>-linux-<arch>.flatpak`
and launch it with `flatpak run com.theri.earthcraft`. The bundle points to Flathub
for its GNOME runtime. It permits network access, graphics and native file dialogs
with host file access. Flatpak manages its application data separately; the bundle
does not include the optional `espeak` narration executable.

Snap bundles use strict confinement with the GNOME extension, network, home,
removable-media and audio interfaces. Install with
`sudo snap install --dangerous ./earthcraft-<version>-linux-<arch>.snap`.
Access to removable media may require `sudo snap connect earthcraft:removable-media`.
These files are sideloadable releases, not publication to Flathub or the Snap Store.

The release matrix configures these packages; native startup, dialogs,
installation, upgrade/uninstall and updater behavior must still be checked on
actual target systems before platform support is announced.

## In-app updates

After the window initializes, EarthCraft checks the public
`therichronicles/EarthCraft` GitHub Releases repository once for a newer stable
version with an exact asset for its platform. A small **Update to vX.Y.Z** button
appears centered between tile status and the heading in the bottom bar. Nothing
is downloaded until clicked. Clicking downloads, verifies GitHub's SHA-256 asset
digest and installs that exact release; failures remain in the control's tooltip.
Right-click a failed update to open the official release page.

Windows/Linux/FreeBSD portable builds replace the executable. macOS uses the updater ZIP
and replaces the entire `EarthCraft.app` bundle; `codesign --verify --deep --strict`
checks the extracted bundle before replacement. The existing DMG remains the
first-install download. The ZIP contains the same ad-hoc signed bundle as the DMG;
checksums and ad-hoc signing do not establish independent publisher authenticity.
No trusted signing key or attestation verification is configured.

After installation, **Restart to update** appears. Clicking it offers the existing
save dialogs for a nonempty places tree and any loaded custom planet. Canceling
leaves the old process running. Saves run on the existing file service while editing
is temporarily disabled; a failed save prevents restart. Once file work finishes,
the normal window closes, preferences flush and workers stop, then the replacement
starts with the same arguments. A failed restart reopens the window with an English
error and the saved documents. Ruler history follows normal quit behavior and is
not persisted. An ordinary quit and relaunch also activates the installed update.

Debug builds and executables running from Cargo's `target` directory never check
for updates, regardless of their version. This includes `cargo run --release`.
Release builds installed outside Cargo output are eligible for checking.
Known package-manager/system locations and macOS apps on a mounted volume or
under App Translocation use a release-page link instead of replacement. Move a macOS app into a writable application folder first. A
protected install also falls back to the release page without elevation. Package
maintainers should set `EARTHCRAFT_UPDATE_MODE=manual` for managed installations in
other locations; `EARTHCRAFT_UPDATE_MODE=off` disables the startup check entirely.
Missing target assets hide the control; no periodic polling or prerelease opt-in is
added. Older macOS releases without the updater ZIP cannot be installed in-app.

Portable replacement/restart still requires native Windows/Linux/FreeBSD validation before
announcing support; packaging/unit checks alone do not verify that lifecycle.

## First launch on macOS (Gatekeeper)

The macOS app is ad-hoc signed but is not notarized or signed with an Apple
Developer ID certificate. To open a release downloaded from this repository:

1. Open the DMG and drag EarthCraft onto the Applications shortcut.
2. Open EarthCraft from Applications once. If Gatekeeper blocks it, dismiss
   the warning without moving the app to Trash.
3. Open **System Settings → Privacy & Security**, scroll to the security
   section and select **Open Anyway** beside the EarthCraft notice.
4. Authenticate if requested and confirm **Open**. Later launches use the
   exception saved for this app.

On older macOS versions, the equivalent controls are under **System Preferences
→ Security & Privacy → General**. See [Apple's opening instructions](https://support.apple.com/en-us/102445)
for the current flow. This creates a per-app exception; disabling Gatekeeper
system-wide is not required.

## Preview builds

Pull requests changing the release workflow, packaging code/tests, changelog
configuration, macOS icon or this guide run
six native architecture builds, two Windows installer jobs and four Linux
bundle jobs, and upload preview artifacts without publishing a release. DEB, RPM,
AppImage and MSI reuse the binaries built by the main matrix. Flatpak and Snap
build the committed source in their own packaging environments. WiX runs
on an x64 Windows host even when packaging an ARM64 executable.
After merge, the Actions page also offers a manual **Release** run with the same
preview-only behavior. Inspect and test these artifacts before creating a version tag.
Actions artifact names also include the workspace version, so their downloadable
wrapper ZIPs are versioned (for example,
`earthcraft-0.1.0-x86_64-pc-windows-msvc.zip`). The release package inside retains
its friendly platform name from the table above.

## Publish a version

1. Set `workspace.package.version` in `Cargo.toml`, regenerate `Cargo.lock`,
   run the contributor checks, and merge those changes.
2. From the reviewed commit, create and push the matching tag, for example
   `git tag v0.1.0` followed by `git push upstream v0.1.0`.
3. The workflow requires the tag to equal `v` plus the workspace version.
   All six native builds, all package formats and asset checksums must
   succeed before publication.
4. Publication first creates a draft with GitHub-generated release notes, uploads the complete
   artifact set, then publishes it. Versions containing a prerelease suffix are
   marked as prereleases.

A failed upload leaves a draft that can be completed by rerunning the workflow.
A published release is not overwritten by a rerun; use a new version and tag.
Only the tag-triggered publication job has `contents: write`; build and preview
jobs have read-only repository access. No signing credentials or additional
personal access token are needed.

## Repository prerequisites

- Merge this workflow into the repository that will publish the release before
  running it manually or pushing a tag. Fork builds publish only to that fork.
- Enable GitHub Actions and permit the actions referenced in the workflow,
  including `dtolnay/rust-toolchain` and `orhun/git-cliff-action`.
  Repository/organization policy must allow the publication job's
  `contents: write` token permission.
- Ensure hosted ARM64 runners (`ubuntu-22.04-arm`, `windows-11-arm`) and the
  macOS runners in the matrix are available to the repository. Jobs queued
  indefinitely require checking runner availability, billing and Actions policy.
- Allow the `vmactions/freebsd-vm@v1` action and nested virtualization on the
  Ubuntu host used for FreeBSD. That job installs Rust 1.95.0 inside the VM.
- Install cargo-packager 0.11.8 for DEB, AppImage, x64 MSI and the macOS app bundle.
  Its AppImage backend uses linuxdeploy with the GTK plugin configured in
  `packaging/cargo-packager.json`; its x64 MSI backend supplies WiX tooling.
  ARM64 MSI uses the official WiX 5.0.2 .NET tool.
  RPM uses cargo-generate-rpm 0.21.0 and the app manifest's metadata.
  Flatpak uses the maintained `org.flatpak.Builder` from Flathub (Ubuntu 22.04's
  native builder expects the SDK's removed `appstream-compose` command),
  GNOME Platform/SDK 49 and the Rust SDK extension
  for Freedesktop 25.08. Snap uses Snapcraft's Rust plugin with Rust 1.95.0,
  core22 and the GNOME extension. These tools are not application dependencies.
- Run the manual preview on the reviewed commit and verify all 22 assets,
  their checksums and actual application startup on each platform. A preview
  never publishes. No release tag is needed for it.
- Review generated release notes before tagging. Use a new matching version/tag;
  publication refuses to overwrite an already published release.

No crates.io publication, Apple signing secrets or Windows certificate are
required by this workflow. Trusted macOS distribution would additionally require
Developer ID signing and notarization, which this workflow does not configure.

## Generated changelog

The workflow runs git-cliff 2.14.2 through `orhun/git-cliff-action@v4` using
`cliff.toml` and full Git history. This is release tooling, not an application
dependency. It groups conventional commits and retains other commit subjects,
while skipping merge boilerplate. Only `vX.Y.Z`-style release tags define release
boundaries; legacy tags such as `2` and `versions` are ignored.

The `release-changelog` Actions artifact contains the generated `CHANGELOG.md`
with the full history; it is not included in application downloads. This is a history of changes,
so review it for features that were subsequently reverted. No generated file is
committed back to the repository, and no PR-label convention is required.

The GitHub release page uses GitHub's native generated notes rather than the
commit-grouped Actions changelog. This produces **What's Changed** entries with
PR titles, authors and PR links, a **New Contributors** section when applicable,
and a full comparison link when a previous comparison tag exists. The release title is
`EarthCraft v0.1.0`. Existing draft notes are preserved on a retry.

Maintainers can preview the GitHub notes without creating a release:

```sh
gh api --method POST repos/therichronicles/EarthCraft/releases/generate-notes -f tag_name=v0.1.0 -f target_commitish=main --jq .body
```

To preview locally, install git-cliff 2.14.2 and run:

```sh
git-cliff --config cliff.toml --output /tmp/earthcraft-CHANGELOG.md
python3 -m unittest discover -s scripts -p 'test_package_*.py'
```

## Local packaging

Python 3.12+ is required for the packaging helper. For a native Linux x86-64 build:

```sh
python3 scripts/package_release.py validate --tag v0.1.0
cargo build -p earthcraft --release --no-default-features --locked
python3 scripts/package_release.py package --version 0.1.0 --target x86_64-unknown-linux-gnu --binary target/release/earthcraft --output /tmp/earthcraft-release
```

Use the target and executable name for your native platform. For explicit-target
builds, the executable lives under `target/<target>/release/`.
The helper packages binaries; it does not cross-compile them. macOS packaging
requires cargo-packager 0.11.8, `codesign`, `hdiutil` and the packaging-only
Python tool `dmgbuild==1.6.7`. cargo-packager constructs the `.app`; the helper
retains numeric release versions, ad-hoc signing and the existing updater ZIP.
It verifies the thin ARM64 executable, signs the app, creates a compressed
read-only DMG and verifies the disk image.

`dmgbuild` writes the Finder layout directly, using the white background and gray
arrow in `assets/packaging/macos/dmg-background{,@2x}.png` and an `/Applications` symlink. No
Finder automation or app runtime dependency is needed. The standard and Retina
backgrounds are checked-in image assets; packaging uses them directly.
To reproduce the Apple Silicon macOS artifact:

```sh
cargo install cargo-packager --version 0.11.8 --locked
python3 -m venv /tmp/earthcraft-packaging
/tmp/earthcraft-packaging/bin/python -m pip install dmgbuild==1.6.7
rustup target add aarch64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=11.0 cargo build -p earthcraft --release --no-default-features --locked --target aarch64-apple-darwin
/tmp/earthcraft-packaging/bin/python scripts/package_release.py package --version 0.1.0 --target aarch64-apple-darwin --binary target/aarch64-apple-darwin/release/earthcraft --output /tmp/earthcraft-release
```

### Additional native formats

The base helper still creates the original portable archives. To add native Linux
formats, install the pinned packaging tools, then run:

```sh
cargo install cargo-packager --version 0.11.8 --locked
cargo install cargo-generate-rpm --version 0.21.0 --locked
python3 scripts/package_linux.py --version 0.1.0 --target x86_64-unknown-linux-gnu --binary target/release/earthcraft --output /tmp/earthcraft-release --formats deb rpm appimage
```

Flatpak and Snap use the same helper with `--formats flatpak` or `--formats snap`,
without `--binary`. They build `git archive HEAD`, so commit source changes before
packaging. Flatpak vendors the locked Cargo dependencies before invoking
flatpak-builder offline with `packaging/flatpak.json`. Install `org.flatpak.Builder`,
its SDK/runtime and the Rust extension into the system Flatpak installation as
in the workflow. The helper runs the sandboxed builder under `dbus-run-session`
so its host Flatpak commands work on headless runners. Snap uses `packaging/snapcraft.json`, a
JSON representation of snapcraft.yaml, and the official Rust plugin with locked,
CPU-only builds. The template omits `version`: the helper injects the workspace
version supplied by the release workflow, so `Cargo.toml` is the single source.
Snapcraft's Rust plugin supplies `--locked`; do not repeat it in extra Cargo
parameters. Snapcraft and restoration of its generated files' ownership use
sudo on the disposable CI host, including cleanup after a failed build.
Both formats compile their own binaries rather than wrapping the portable archive.

On Windows, install cargo-packager 0.11.8 for x64 or run
`dotnet tool install --global wix --version 5.0.2` for ARM64, then run `scripts/package_windows.py`
with the same arguments as the base helper. It adds MSI packages and the x64
standalone EXE. FreeBSD's base helper uses `--target x86_64-unknown-freebsd` after
a native CPU-only build. The release asset names and updater archive formats
are unchanged by the packaging tools.

Before consolidating checksum sidecars, validate a complete downloaded artifact set:

```sh
python3 scripts/package_release.py verify --version 0.1.0 --output /tmp/earthcraft-release
```
