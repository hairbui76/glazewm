//! Self-update support backed by GitHub releases.
//!
//! Only available on Windows, since the release pipeline currently only
//! publishes Windows installers.

use std::{
  path::{Path, PathBuf},
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use anyhow::Context;
use semver::Version;
use serde::Deserialize;
use tokio::sync::mpsc;
use wm_platform::Dispatcher;

/// GitHub repository (in `owner/repo` format) that releases are pulled
/// from.
///
/// Overridable at build time via the `UPDATE_REPO` environment variable.
const UPDATE_REPO: &str = match option_env!("UPDATE_REPO") {
  Some(repo) => repo,
  None => "hairbui76/glazewm",
};

/// Timeout for the GitHub API request that resolves the latest release.
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for downloading the installer.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(10);

/// Maximum time that the deferred installer waits for the WM process to
/// exit before giving up.
const EXIT_WAIT_SECS: u32 = 60;

/// Whether an update check is currently running.
///
/// Update checks are triggered from the system tray, which allows the menu
/// item to be clicked repeatedly. Only one check is allowed at a time.
static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Release payload returned by the GitHub releases API.
#[derive(Debug, Deserialize)]
struct GithubRelease {
  tag_name: String,
  assets: Vec<GithubAsset>,
}

/// Release asset returned by the GitHub releases API.
#[derive(Debug, Deserialize)]
struct GithubAsset {
  name: String,
  browser_download_url: String,
}

/// Latest release available on GitHub.
#[derive(Debug)]
struct LatestRelease {
  version: Version,
  installer_name: String,
  installer_url: String,
}

/// Checks for a newer release in the background and, if the user agrees,
/// downloads and installs it.
///
/// Returns immediately. The check runs on a dedicated thread so that the
/// system tray remains responsive while the network request and download
/// are in-flight. Failures are surfaced to the user via an error dialog.
pub fn check_for_updates(
  dispatcher: &Dispatcher,
  exit_tx: &mpsc::UnboundedSender<()>,
) {
  if UPDATE_IN_PROGRESS
    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
    .is_err()
  {
    tracing::info!("An update check is already in progress.");
    return;
  }

  let dispatcher = dispatcher.clone();
  let exit_tx = exit_tx.clone();

  std::thread::spawn(move || {
    if let Err(err) = run_update_flow(&dispatcher, &exit_tx) {
      tracing::error!("Update check failed: {:?}", err);
      dispatcher.show_error_dialog(
        "Unable to update GlazeWM",
        &format!("{err:#}"),
      );
    }

    UPDATE_IN_PROGRESS.store(false, Ordering::SeqCst);
  });
}

/// Runs the full update flow: resolve the latest release, prompt the user,
/// download the installer, and schedule it to run after exit.
fn run_update_flow(
  dispatcher: &Dispatcher,
  exit_tx: &mpsc::UnboundedSender<()>,
) -> anyhow::Result<()> {
  let current_version = current_version()?;
  let release = fetch_latest_release()?;

  tracing::info!(
    "Latest release is v{}, current version is v{current_version}.",
    release.version
  );

  if release.version <= current_version {
    dispatcher.show_info_dialog(
      "GlazeWM is up to date",
      &format!("You're running the latest version (v{current_version})."),
    );

    return Ok(());
  }

  let should_update = dispatcher.show_confirm_dialog(
    "Update available",
    &format!(
      "GlazeWM v{} is available. You're currently on v{current_version}.\n\n\
       Download and install it now? GlazeWM exits while the installer \
       runs and is relaunched once it completes.",
      release.version
    ),
  );

  if !should_update {
    tracing::info!("Update to v{} declined by user.", release.version);
    return Ok(());
  }

  let installer_path = download_installer(&release)?;
  run_installer_after_exit(&installer_path)?;

  tracing::info!("Exiting to install v{}.", release.version);
  exit_tx
    .send(())
    .context("Failed to signal exit for the pending update.")?;

  Ok(())
}

/// Parses the version that the application was built with.
fn current_version() -> anyhow::Result<Version> {
  Version::parse(env!("VERSION_NUMBER")).with_context(|| {
    format!("Invalid current version '{}'.", env!("VERSION_NUMBER"))
  })
}

/// Fetches the latest release from the GitHub releases API.
///
/// Draft and pre-releases are excluded by the API. Returns an error if the
/// release does not (yet) have a Windows installer attached, which can
/// happen in the window between a release being published and its assets
/// being uploaded.
fn fetch_latest_release() -> anyhow::Result<LatestRelease> {
  let url =
    format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");

  let body = http_agent(METADATA_TIMEOUT)
    .get(&url)
    .header("Accept", "application/vnd.github+json")
    .header("X-GitHub-Api-Version", "2022-11-28")
    .call()
    .with_context(|| format!("Failed to query '{url}'."))?
    .body_mut()
    .read_to_string()
    .context("Failed to read the GitHub API response.")?;

  let release = serde_json::from_str::<GithubRelease>(&body)
    .context("Failed to parse the GitHub API response.")?;

  let version = parse_release_version(&release.tag_name)?;

  let asset = installer_asset(&release.assets).with_context(|| {
    format!(
      "Release {} has no Windows installer attached yet. Try again in a \
       few minutes.",
      release.tag_name
    )
  })?;

  Ok(LatestRelease {
    version,
    installer_name: asset.name.clone(),
    installer_url: asset.browser_download_url.clone(),
  })
}

/// Downloads the release's installer into a temporary directory.
///
/// The download is written to a `.part` file that is only renamed on
/// success, so that an interrupted download is never mistaken for a
/// complete installer.
///
/// Returns the path of the downloaded installer.
fn download_installer(release: &LatestRelease) -> anyhow::Result<PathBuf> {
  let download_dir = std::env::temp_dir().join("glazewm-update");
  std::fs::create_dir_all(&download_dir).with_context(|| {
    format!("Failed to create '{}'.", download_dir.display())
  })?;

  let installer_path = download_dir.join(&release.installer_name);
  let partial_path = installer_path.with_extension("part");

  tracing::info!(
    "Downloading '{}' to '{}'.",
    release.installer_url,
    installer_path.display()
  );

  let mut reader = http_agent(DOWNLOAD_TIMEOUT)
    .get(&release.installer_url)
    .call()
    .with_context(|| {
      format!("Failed to download '{}'.", release.installer_url)
    })?
    .into_body()
    .into_reader();

  {
    let mut file =
      std::fs::File::create(&partial_path).with_context(|| {
        format!("Failed to create '{}'.", partial_path.display())
      })?;

    std::io::copy(&mut reader, &mut file)
      .context("Failed to write the downloaded installer.")?;
  }

  std::fs::rename(&partial_path, &installer_path).with_context(|| {
    format!("Failed to finalize '{}'.", installer_path.display())
  })?;

  Ok(installer_path)
}

/// Schedules the installer to run once the current process has exited.
///
/// The installer overwrites `glazewm.exe`, so it cannot run while the WM
/// is still alive. A detached PowerShell process is spawned to poll for
/// the WM's exit, run the installer, and relaunch `GlazeWM` afterwards.
///
/// The spawned process is created without a console window so that it
/// stays invisible to the user.
fn run_installer_after_exit(installer_path: &Path) -> anyhow::Result<()> {
  use std::os::windows::process::CommandExt;

  /// Creation flag that prevents a console window from being allocated.
  ///
  /// Ref: <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
  const CREATE_NO_WINDOW: u32 = 0x0800_0000;

  let exe_path = std::env::current_exe()
    .context("Failed to resolve the current executable path.")?;

  let script =
    deferred_install_script(std::process::id(), installer_path, &exe_path);

  std::process::Command::new("powershell")
    .args([
      "-NoProfile",
      "-NonInteractive",
      "-WindowStyle",
      "Hidden",
      "-Command",
      &script,
    ])
    .creation_flags(CREATE_NO_WINDOW)
    .spawn()
    .context("Failed to spawn the deferred installer process.")?;

  Ok(())
}

/// Builds the PowerShell script that waits for the WM process to exit,
/// runs the installer, and relaunches the WM.
///
/// The installer is run in passive mode so that the user sees its progress
/// without having to click through it. `GlazeWM` is only relaunched if the
/// installer succeeded, where exit code `3010` means that it succeeded but
/// wants a reboot.
fn deferred_install_script(
  pid: u32,
  installer_path: &Path,
  exe_path: &Path,
) -> String {
  let installer = escape_ps_literal(&installer_path.to_string_lossy());
  let exe = escape_ps_literal(&exe_path.to_string_lossy());

  format!(
    "$deadline = (Get-Date).AddSeconds({EXIT_WAIT_SECS}); \
     while ((Get-Process -Id {pid} -ErrorAction SilentlyContinue) -and \
     ((Get-Date) -lt $deadline)) {{ Start-Sleep -Milliseconds 200 }}; \
     if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 1 }}; \
     $proc = Start-Process -FilePath '{installer}' \
     -ArgumentList '/passive','/norestart' -PassThru -Wait; \
     if (($proc.ExitCode -eq 0) -or ($proc.ExitCode -eq 3010))      {{ Start-Process -FilePath '{exe}' }}"
  )
}

/// Escapes a value for use within a single-quoted PowerShell string.
fn escape_ps_literal(value: &str) -> String {
  value.replace('\'', "''")
}

/// Creates an HTTP agent with the given global timeout.
fn http_agent(timeout: Duration) -> ureq::Agent {
  let config = ureq::Agent::config_builder()
    .user_agent(concat!("GlazeWM/", env!("VERSION_NUMBER")))
    .timeout_global(Some(timeout))
    .build();

  ureq::Agent::new_with_config(config)
}

/// Parses the semantic version out of a release tag (e.g. `v1.2.3`).
fn parse_release_version(tag_name: &str) -> anyhow::Result<Version> {
  Version::parse(tag_name.trim_start_matches('v'))
    .with_context(|| format!("Invalid release tag '{tag_name}'."))
}

/// Finds the universal Windows installer within a release's assets.
///
/// The installer is named `glazewm-v<version>.exe` by the release
/// pipeline, which distinguishes it from the standalone MSI's.
fn installer_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
  assets.iter().find(|asset| {
    asset.name.starts_with("glazewm-v")
      && Path::new(&asset.name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
  })
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::{
    deferred_install_script, escape_ps_literal, installer_asset,
    parse_release_version, GithubAsset, GithubRelease,
  };

  #[test]
  fn parses_release_version_with_and_without_prefix() {
    assert_eq!(
      parse_release_version("v3.11.0").unwrap().to_string(),
      "3.11.0"
    );
    assert_eq!(
      parse_release_version("3.11.0").unwrap().to_string(),
      "3.11.0"
    );
    assert!(parse_release_version("nightly").is_err());
  }

  #[test]
  fn selects_universal_installer_from_assets() {
    let release = serde_json::from_str::<GithubRelease>(
      r#"{
        "tag_name": "v3.11.0",
        "assets": [
          {
            "name": "standalone-glazewm-v3.11.0-x64.msi",
            "browser_download_url": "https://example.com/x64.msi"
          },
          {
            "name": "glazewm-v3.11.0.exe",
            "browser_download_url": "https://example.com/universal.exe"
          }
        ]
      }"#,
    )
    .unwrap();

    let asset = installer_asset(&release.assets).unwrap();
    assert_eq!(asset.name, "glazewm-v3.11.0.exe");
    assert_eq!(
      asset.browser_download_url,
      "https://example.com/universal.exe"
    );
  }

  #[test]
  fn ignores_releases_without_an_installer() {
    let assets = vec![GithubAsset {
      name: "standalone-glazewm-v3.11.0-arm64.msi".to_string(),
      browser_download_url: "https://example.com/arm64.msi".to_string(),
    }];

    assert!(installer_asset(&assets).is_none());
  }

  #[test]
  fn escapes_single_quotes_in_ps_literals() {
    assert_eq!(escape_ps_literal(r"C:\dev\it's"), r"C:\dev\it''s");
    assert_eq!(escape_ps_literal(r"C:\dev"), r"C:\dev");
  }

  #[test]
  fn deferred_script_quotes_paths_and_waits_on_pid() {
    let script = deferred_install_script(
      1234,
      Path::new(r"C:\temp\o'brien\glazewm-v3.11.0.exe"),
      Path::new(r"C:\Program Files\glzr.io\glazewm.exe"),
    );

    assert!(script.contains("-Id 1234"));
    assert!(
      script.contains(r"'C:\temp\o''brien\glazewm-v3.11.0.exe'"),
      "installer path should be single-quoted and escaped"
    );
    assert!(
      script.contains(r"'C:\Program Files\glzr.io\glazewm.exe'"),
      "executable path should be single-quoted"
    );
  }
}
