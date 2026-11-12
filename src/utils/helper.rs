use anyhow::{Context, Result};
use log::{Log, Metadata, Record, debug, info, warn};
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

pub fn web_url(host: &str, port: u16) -> String {
    if port == 80 {
        format!("http://{host}/")
    } else {
        format!("http://{host}:{port}/")
    }
}

/// For 0.0.0.0 the LAN address is found via UDP sockets "connected" to a few targets (no packet sent).
/// The later targets are private ranges, so they work without an uplink as long as some interface has a route.
pub fn lan_host(bind_address: &str) -> Option<String> {
    if bind_address != "0.0.0.0" {
        return Some(bind_address.to_string());
    }
    [
        "8.8.8.8:80",
        "10.255.255.255:1",
        "192.168.255.255:1",
        "172.31.255.255:1",
    ]
    .iter()
    .find_map(|target| {
        let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        sock.connect(target).ok()?;
        let ip = sock.local_addr().ok()?.ip();
        (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
    })
}

/// Text after "Web remote: " in the TUI; "off" only when the API is not started.
pub fn web_remote_line(api_on: bool, lan_url: Option<&str>, port: u16) -> String {
    match (api_on, lan_url) {
        (false, _) => "off".to_string(),
        (true, Some(u)) => u.to_string(),
        (true, None) => format!("on, port {port} (LAN IP unknown)"),
    }
}

pub fn open_in_browser(url: &str) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    // Spawn without waiting so the TUI never blocks.
    cmd.arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Err(e) = cmd.spawn() {
        warn!("Could not open browser: {e}");
    }
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
        assert_eq!(
            installer_url(&json!({ "message": "API rate limit exceeded" })),
            None
        );
    }

    #[test]
    fn web_url_omits_port_80() {
        assert_eq!(web_url("192.168.1.5", 80), "http://192.168.1.5/");
    }

    #[test]
    fn web_url_keeps_other_port() {
        assert_eq!(web_url("localhost", 8080), "http://localhost:8080/");
    }

    #[test]
    fn lan_host_keeps_explicit_bind() {
        assert_eq!(lan_host("127.0.0.1"), Some("127.0.0.1".to_string()));
    }

    #[test]
    fn web_remote_line_distinguishes_off_and_unknown_ip() {
        assert_eq!(web_remote_line(false, None, 8080), "off");
        assert_eq!(
            web_remote_line(true, Some("http://1.2.3.4:8080/"), 8080),
            "http://1.2.3.4:8080/"
        );
        assert_eq!(
            web_remote_line(true, None, 8080),
            "on, port 8080 (LAN IP unknown)"
        );
    }
}
