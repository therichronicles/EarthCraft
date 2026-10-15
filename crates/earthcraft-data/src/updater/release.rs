use super::{Snapshot, UpdateState};
use self_update::{
    Release, ReleaseAsset,
    backends::github::{AsyncUpdate, Update},
};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::watch;
const UPDATE_TIMEOUT_SECS: u64 = 30;
const UPDATE_RETRIES: u32 = 1;
const REPO_OWNER: &str = "therichronicles";
const REPO_NAME: &str = "EarthCraft";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub(super) struct SelectedRelease {
    pub(super) version: String,
    tag: String,
    asset: ReleaseAsset,
}
fn platform_suffix() -> Option<&'static str> {
    platform_suffix_for(std::env::consts::OS, std::env::consts::ARCH)
}
fn platform_suffix_for(os: &str, architecture: &str) -> Option<&'static str> {
    match (os, architecture) {
        ("linux", "x86_64") => Some("linux-x64.tar.gz"),
        ("linux", "aarch64") => Some("linux-arm64.tar.gz"),
        ("freebsd", "x86_64") => Some("freebsd-x64.tar.gz"),
        ("windows", "x86_64") => Some("windows-x64-portable.zip"),
        ("windows", "aarch64") => Some("windows-arm64-portable.zip"),
        ("macos", "aarch64") => Some("macos-arm64-update.zip"),
        _ => None,
    }
}
/// Cargo output is a development installation, including `cargo run --release`.
pub(super) fn is_development_path(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str() == "target")
}

pub(super) fn manual_install_reason(path: &Path) -> Option<String> {
    let managed = path.components().any(|part| {
        matches!(
            part.as_os_str().to_str(),
            Some(
                "target"
                    | ".cargo"
                    | "Cellar"
                    | "Homebrew"
                    | "homebrew"
                    | "snap"
                    | "flatpak"
                    | "WindowsApps"
                    | "Program Files"
                    | "Program Files (x86)"
            )
        )
    }) || ["/usr", "/bin", "/sbin", "/opt", "/nix", "/app"]
        .iter()
        .any(|root| path.starts_with(root));
    let unsuitable_bundle = cfg!(target_os = "macos")
        && (!path.ends_with("EarthCraft.app/Contents/MacOS/earthcraft")
            || path.starts_with("/Volumes")
            || path
                .components()
                .any(|part| part.as_os_str() == "AppTranslocation"));
    if unsuitable_bundle {
        Some("Move EarthCraft.app to a writable application folder and relaunch before updating. Open Releases for a manual download.".into())
    } else if managed || cfg!(debug_assertions) {
        Some("Development and managed installations must be updated through their original build or package manager. Open Releases for portable downloads.".into())
    } else {
        None
    }
}
fn builder() -> self_update::backends::github::UpdateBuilder {
    let mut builder = Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .tag_prefix("v")
        .bin_name(if cfg!(windows) {
            "earthcraft.exe"
        } else {
            "earthcraft"
        })
        .current_version(VERSION)
        .no_confirm(true)
        .show_output(false)
        .show_download_progress(false)
        .timeout(Duration::from_secs(UPDATE_TIMEOUT_SECS))
        .retries(UPDATE_RETRIES);
    #[cfg(target_os = "macos")]
    builder.bundle_path_in_archive("EarthCraft.app");
    builder
}
fn checker() -> self_update::Result<AsyncUpdate> {
    builder().build_async()
}

pub(super) async fn check() -> self_update::Result<Option<SelectedRelease>> {
    // GitHub's /latest endpoint excludes drafts and prereleases. Check semver too.
    let Some(release) = checker()?.is_update_available_async().await? else {
        return Ok(None);
    };
    select_release(&release, VERSION, platform_suffix())
}
fn select_release(
    release: &Release,
    current: &str,
    suffix: Option<&str>,
) -> self_update::Result<Option<SelectedRelease>> {
    let version = semver::Version::parse(release.version())
        .map_err(|error| self_update::Error::archive_verification_rejected(error.to_string()))?;
    let current = semver::Version::parse(current)
        .map_err(|error| self_update::Error::archive_verification_rejected(error.to_string()))?;
    if !version.pre.is_empty() || version <= current {
        return Ok(None);
    }
    let Some(suffix) = suffix else {
        return Ok(None);
    };
    let expected = format!("earthcraft-{version}-{suffix}");
    let Some(asset) = release
        .assets()
        .iter()
        .find(|asset| asset.name() == expected)
    else {
        return Ok(None);
    };
    validate_asset(asset)?;
    Ok(Some(SelectedRelease {
        version: version.to_string(),
        tag: format!("v{version}"),
        asset: asset.clone(),
    }))
}
fn same_asset(left: &ReleaseAsset, right: &ReleaseAsset) -> bool {
    left.name() == right.name()
        && left.download_url() == right.download_url()
        && left.digest() == right.digest()
}
fn validate_asset(asset: &ReleaseAsset) -> self_update::Result<()> {
    let origin = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/assets/");
    if !asset
        .download_url()
        .strip_prefix(&origin)
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(self_update::Error::archive_verification_rejected(
            "Unexpected release asset origin",
        ));
    }
    if !asset
        .digest()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(self_update::Error::archive_verification_rejected(
            "Release asset has no valid GitHub SHA-256 digest",
        ));
    }
    Ok(())
}
pub(super) async fn install(
    selected: &SelectedRelease,
    sender: watch::Sender<Snapshot>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) -> self_update::Result<bool> {
    // Re-fetch the exact displayed tag; never install whatever became latest later.
    let release = checker()?.get_release_version_async(&selected.tag).await?;
    let fresh = select_release(&release, VERSION, platform_suffix())?.ok_or_else(|| {
        self_update::Error::archive_verification_rejected("Selected update is no longer available")
    })?;
    if fresh.tag != selected.tag || !same_asset(&fresh.asset, &selected.asset) {
        return Err(self_update::Error::archive_verification_rejected(
            "Selected release asset changed after checking",
        ));
    }
    let expected = selected.asset.clone();
    let mut builder = builder();
    builder
        .release_tag(&selected.tag)
        .check_install_path_writable(true)
        .asset_matcher(move |assets| {
            assets
                .iter()
                .find(|asset| same_asset(asset, &expected))
                .cloned()
        });
    let progress_selected = selected.clone();
    let progress_sender = sender.clone();
    let progress_repaint = repaint.clone();
    builder.progress_callback(move |downloaded, total| {
        progress_sender.send_replace(Snapshot {
            state: UpdateState::Downloading {
                version: progress_selected.version.clone(),
                downloaded,
                total,
            },
            selected: Some(progress_selected.clone()),
        });
        progress_repaint();
    });
    #[cfg(target_os = "macos")]
    builder.verify_binary(|bundle| {
        let output = std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(self_update::Error::verification_rejected(format!(
                "macOS bundle signature validation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    });
    let selected = selected.clone();
    builder.verify_archive(move |_| {
        sender.send_replace(Snapshot {
            state: UpdateState::Installing {
                version: selected.version.clone(),
            },
            selected: Some(selected.clone()),
        });
        repaint();
        Ok(())
    });
    Ok(builder.build_async()?.update_async().await?.is_updated())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cargo_debug_and_release_outputs_are_development_installations() {
        for path in [
            "/project/target/debug/earthcraft",
            "/project/target/release/earthcraft",
            "/project/target/aarch64-apple-darwin/release/EarthCraft.app/Contents/MacOS/earthcraft",
        ] {
            assert!(is_development_path(Path::new(path)));
        }
        assert!(!is_development_path(Path::new(
            "/Applications/EarthCraft.app/Contents/MacOS/earthcraft"
        )));
        assert!(!is_development_path(Path::new(
            "/home/user/Downloads/earthcraft"
        )));
    }
    fn release(version: &str, asset: ReleaseAsset) -> Release {
        Release::builder()
            .name("fixture")
            .version(version)
            .date("2026-10-14")
            .assets(vec![asset])
            .build()
            .unwrap()
    }
    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset::new(
            name,
            "https://api.github.com/repos/therichronicles/EarthCraft/releases/assets/123",
        )
        .with_digest(format!("sha256:{}", "a".repeat(64)))
    }
    #[test]
    fn selection_preserves_stable_channel_exact_target_and_semver_order() {
        let newer = release("0.10.0", asset("earthcraft-0.10.0-linux-x64.tar.gz"));
        let selected = select_release(&newer, "0.9.0", Some("linux-x64.tar.gz"))
            .unwrap()
            .unwrap();
        assert_eq!(selected.tag, "v0.10.0");
        assert!(
            select_release(&newer, "0.10.0", Some("linux-x64.tar.gz"))
                .unwrap()
                .is_none()
        );
        assert!(
            select_release(&newer, "0.9.0", Some("windows-x64-portable.zip"))
                .unwrap()
                .is_none()
        );
        assert!(select_release(&newer, "0.9.0", None).unwrap().is_none());
        let prerelease = release(
            "1.0.0-beta.1",
            asset("earthcraft-1.0.0-beta.1-linux-x64.tar.gz"),
        );
        assert!(
            select_release(&prerelease, "0.9.0", Some("linux-x64.tar.gz"))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn selection_rejects_untrusted_urls_and_missing_digests() {
        let bad = ReleaseAsset::new(
            "earthcraft-1.0.0-linux-x64.tar.gz",
            "https://example.com/update",
        );
        assert!(select_release(&release("1.0.0", bad), "0.1.0", Some("linux-x64.tar.gz")).is_err());
        let missing = ReleaseAsset::new(
            "earthcraft-1.0.0-linux-x64.tar.gz",
            "https://api.github.com/repos/therichronicles/EarthCraft/releases/assets/123",
        );
        assert!(
            select_release(
                &release("1.0.0", missing),
                "0.1.0",
                Some("linux-x64.tar.gz")
            )
            .is_err()
        );
    }
    #[test]
    fn added_native_targets_select_their_own_portable_archives() {
        assert_eq!(platform_suffix_for("windows", "x86"), None);
        assert_eq!(
            platform_suffix_for("freebsd", "x86_64"),
            Some("freebsd-x64.tar.gz")
        );
        assert_eq!(
            platform_suffix_for("macos", "aarch64"),
            Some("macos-arm64-update.zip")
        );
        assert_eq!(platform_suffix_for("macos", "x86_64"), None);
    }
    #[test]
    fn managed_locations_never_enable_binary_replacement() {
        for path in [
            "/usr/bin/earthcraft",
            "/opt/earthcraft",
            "/home/user/.cargo/bin/earthcraft",
            "/repo/target/release/earthcraft",
        ] {
            assert!(manual_install_reason(Path::new(path)).is_some());
        }
    }
}
