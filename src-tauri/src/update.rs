//! Looks for a newer VOGT on the update channel the settings name, and installs it.

use serde::Deserialize;
use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_updater::{Update, UpdaterExt};
use ts_rs::TS;

/// Where the channel manifests are published; `.github/workflows/pages.yml`
/// writes them. Every shipped build asks this address, so it must never move.
const MANIFESTS: &str = "https://vogt.phyrone.de/updates";

/// Which releases the app updates to.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Channel {
    /// Full releases only.
    Stable,
    /// Prereleases too; a full release lands here as well.
    Beta,
}

impl Channel {
    /// The manifest that names the channel's newest release.
    fn manifest(self) -> Url {
        let name = match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        };
        Url::parse(&format!("{MANIFESTS}/{name}.json")).expect("the manifest address is a URL")
    }
}

/// The release `channel` offers above the running version, if any. The
/// endpoint is set here rather than in `tauri.conf.json` because it depends on
/// the channel.
async fn find<R: Runtime>(
    app: &AppHandle<R>,
    channel: Channel,
) -> tauri_plugin_updater::Result<Option<Update>> {
    app.updater_builder()
        .endpoints(vec![channel.manifest()])?
        .build()?
        .check()
        .await
}

/// The version `channel` offers above the running one, if any.
#[tauri::command]
pub async fn check_update<R: Runtime>(
    app: AppHandle<R>,
    channel: Channel,
) -> Result<Option<String>, String> {
    let update = find(&app, channel).await.map_err(|e| e.to_string())?;
    Ok(update.map(|update| update.version))
}

/// Downloads and installs what `channel` offers, then starts the new version.
/// Returns only when there was nothing to install or it failed.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>, channel: Channel) -> Result<(), String> {
    let Some(update) = find(&app, channel).await.map_err(|e| e.to_string())? else {
        return Ok(());
    };
    // on Windows the installer takes over here and the app exits with it
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_channel_has_its_own_https_manifest() {
        let stable = Channel::Stable.manifest();
        let beta = Channel::Beta.manifest();

        // the updater refuses any other scheme in a release build
        assert_eq!(stable.scheme(), "https");
        assert_eq!(stable.path(), "/updates/stable.json");
        assert_eq!(beta.path(), "/updates/beta.json");
    }
}
