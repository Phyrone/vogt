//! Looks for a newer build of the running flavour of VOGT, and installs it.

use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Where the channel manifests are published; `.github/workflows/pages.yml`
/// writes them. Every shipped build asks this address, so it must never move.
const MANIFESTS: &str = "https://vogt.phyrone.de/updates";

/// The flavour a build is, and with it the only channel it updates from. Each
/// has its own identifier, so the three install side by side and a build is
/// never replaced by one of another flavour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    /// The tagged releases.
    Stable,
    /// The releases tagged ahead of a stable one, to try them first.
    Beta,
    /// The build of the main branch, which is no release at all.
    Canary,
}

impl Channel {
    /// The channel of the build with this identifier. `tauri.beta.conf.json`
    /// and `tauri.canary.conf.json` end theirs in the channel's name; every
    /// other identifier is the stable one.
    fn of(identifier: &str) -> Self {
        match identifier.rsplit('.').next() {
            Some("beta") => Channel::Beta,
            Some("canary") => Channel::Canary,
            _ => Channel::Stable,
        }
    }

    /// The manifest that names the channel's newest build.
    fn manifest(self) -> Url {
        let name = match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Canary => "canary",
        };
        Url::parse(&format!("{MANIFESTS}/{name}.json")).expect("the manifest address is a URL")
    }
}

/// The build the app's channel offers above the running version, if any. The
/// endpoint is set here rather than in `tauri.conf.json` because it depends on
/// the channel.
async fn find<R: Runtime>(app: &AppHandle<R>) -> tauri_plugin_updater::Result<Option<Update>> {
    let channel = Channel::of(&app.config().identifier);
    app.updater_builder()
        .endpoints(vec![channel.manifest()])?
        .build()?
        .check()
        .await
}

/// The version the app's channel offers above the running one, if any.
#[tauri::command]
pub async fn check_update<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    let update = find(&app).await.map_err(|e| e.to_string())?;
    Ok(update.map(|update| update.version))
}

/// Downloads and installs what the app's channel offers, then starts the new
/// version. Returns only when there was nothing to install or it failed.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let Some(update) = find(&app).await.map_err(|e| e.to_string())? else {
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

    /// The identifier a Tauri config file declares.
    fn identifier(config: &str) -> String {
        let config: serde_json::Value = serde_json::from_str(config).unwrap();
        config["identifier"].as_str().unwrap().to_owned()
    }

    #[test]
    fn every_flavour_updates_from_its_own_channel() {
        let stable = identifier(include_str!("../tauri.conf.json"));
        let beta = identifier(include_str!("../tauri.beta.conf.json"));
        let canary = identifier(include_str!("../tauri.canary.conf.json"));

        assert_eq!(Channel::of(&stable), Channel::Stable);
        assert_eq!(Channel::of(&beta), Channel::Beta);
        assert_eq!(Channel::of(&canary), Channel::Canary);
    }

    #[test]
    fn every_channel_has_its_own_https_manifest() {
        let stable = Channel::Stable.manifest();

        // the updater refuses any other scheme in a release build
        assert_eq!(stable.scheme(), "https");
        assert_eq!(stable.path(), "/updates/stable.json");
        assert_eq!(Channel::Beta.manifest().path(), "/updates/beta.json");
        assert_eq!(Channel::Canary.manifest().path(), "/updates/canary.json");
    }
}
