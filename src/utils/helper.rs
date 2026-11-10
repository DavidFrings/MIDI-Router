use anyhow::{Context, Result};
use log::{Log, Metadata, Record, debug, info};
use reqwest::Client;
use serde_json::Value;
use std::{
    env, fs,
    process::{Command, exit},
    sync::{Mutex, mpsc::Sender},
};

pub(crate) struct ForwardLogger {
    pub(crate) tx_router: Mutex<Sender<String>>,
    pub(crate) tx_api: Mutex<Sender<String>>,
}

impl Log for ForwardLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let msg = format!("{}: {}", record.level(), record.args());
        let target = record.target();
        let msg_lower = msg.to_lowercase();

        let is_api_log = target.starts_with("api")
            || target.starts_with("actix")
            || target.starts_with("tokio")
            || msg_lower.contains("actix")
            || msg_lower.contains("endpoint")
            || msg_lower.contains("workers")
            || msg_lower.contains("tokio runtime")
            || msg_lower.contains("api")
            || msg_lower.contains("service:");

        if is_api_log {
            if let Ok(tx) = self.tx_api.lock() {
                let _ = tx.send(msg.clone());
            }
        } else {
            if let Ok(tx) = self.tx_router.lock() {
                let _ = tx.send(msg);
            }
        }
    }
    fn flush(&self) {}
}
const RELEASE_URL: &str = "https://api.github.com/repos/DavidFrings/MIDI-Router/releases/latest";
const INSTALLER_ASSET: &str = "midi-router-installer.exe";
const USER_AGENT: &str = "MIDI-Router-Updater";

/// Downloads and starts the installer of a newer release, then exits.
pub(crate) async fn update() -> Result<()> {
    let client = Client::new();
    let current = env!("CARGO_PKG_VERSION");

    let release: Value = client
        .get(RELEASE_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let latest = release["tag_name"]
        .as_str()
        .context("Release has no tag_name")?;

    if !is_newer_version(latest.trim_start_matches('v'), current) {
        info!("Router is up to date (v{})", current);
        debug!("Latest version: {}", latest);
        return Ok(());
    }

    let download_url = installer_url(&release)
        .with_context(|| format!("Release {} has no {}", latest, INSTALLER_ASSET))?;

    info!("New version available: {} (current: v{})", latest, current);
    debug!("Download url: {}", download_url);

    let installer = client
        .get(download_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let path = env::temp_dir().join(INSTALLER_ASSET);
    fs::write(&path, &installer)?;

    // `start` goes through ShellExecute, so the installer can ask for admin rights (UAC).
    Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(&path)
        .spawn()?;

    exit(0);
}

fn installer_url(release: &Value) -> Option<&str> {
    release["assets"]
        .as_array()?
        .iter()
        .find(|asset| asset["name"] == INSTALLER_ASSET)?["browser_download_url"]
        .as_str()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_parts: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();
    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c {
            return true;
        }

        if l < c {
            return false;
        }
    }

    latest_parts.len() > current_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release() -> Value {
        // Same asset order as the real v0.2.6 release.
        json!({
            "tag_name": "v0.2.6",
            "assets": [
                { "name": "example.config.toml", "browser_download_url": "https://x/example.config.toml" },
                { "name": "midi-router-installer.exe", "browser_download_url": "https://x/midi-router-installer.exe" },
                { "name": "midi-router.exe", "browser_download_url": "https://x/midi-router.exe" }
            ]
        })
    }

    #[test]
    fn installer_is_picked_by_name_not_position() {
        assert_eq!(
            installer_url(&release()),
            Some("https://x/midi-router-installer.exe")
        );
    }

    #[test]
    fn release_without_installer_gives_none() {
        assert_eq!(installer_url(&json!({ "assets": [] })), None);
        assert_eq!(installer_url(&json!({ "message": "API rate limit exceeded" })), None);
    }
}
