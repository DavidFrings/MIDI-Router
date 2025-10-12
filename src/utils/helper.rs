use anyhow::Result;
use log::{Log, Metadata, Record, debug, info};
use reqwest::Client;
use serde_json::Value;
use std::{
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
pub(crate) async fn update() -> Result<()> {
    let client = Client::new();
    let current = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/DavidFrings/MIDI-Router/releases/latest";

    let res: Value = client
        .get(url)
        .header("User-Agent", "MIDI-Router-Updater")
        .send()
        .await?
        .json()
        .await?;

    let latest = res["tag_name"].as_str().unwrap();

    if is_newer_version(latest.trim_start_matches('v'), current) {
        let download_url = res["assets"][0]["browser_download_url"].as_str().unwrap();

        info!("New version available: {} (current: v{})", latest, current);
        debug!("Download url: {}", download_url);

        Command::new("updater.exe")
            .arg(download_url)
            .spawn()
            .expect("Failed to start updater");

        exit(0);
    }

    info!("Router is up to date (v{})", current);
    debug!("Latest version: {}", latest);
    Ok(())
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
