mod router;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::info;
use reqwest::Client;
use router::midi_connection::MidiRouter;
use std::{thread, process::Command, time::Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long, default_value = "APC40 mkII")]
    controller_name: String,

    #[clap(short, long, default_value = "Daslight")]
    software_name: String,
}

async fn update() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/DavidFrings/MIDI-Router/releases/latest";
    let client = Client::new();
    let res: serde_json::Value = client
        .get(url)
        .header("User-Agent", "MIDI-Router-Updater")
        .send()
        .await?
        .json()
        .await?;
    let latest = res["tag_name"].as_str().unwrap();

    if latest != format!("v{}", current_version) {
        let download_url = res["assets"][0]["browser_download_url"].as_str().unwrap();

        info!(
            "New version available: {} (current: v{})",
            latest, current_version
        );

        Command::new("updater.exe")
            .arg(download_url)
            .spawn()
            .expect("Failed to start updater");

        std::process::exit(0);
    }

    info!("Router is up to date (version {})", latest);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Check for updates
    info!("Checking for updates...");
    update().await.expect("Failed to check for updates"); // Disable for dev!

    let args = Args::parse();

    info!("Controller MIDI port: {}", args.controller_name);
    info!("Software MIDI port: {}", args.software_name);

    let mut router = MidiRouter::new();

    info!("Connecting to MIDI ports...");

    MidiRouter::connect(&mut router, &args.controller_name, &args.software_name)?;

    info!("Connected successfully!");
    info!("Router is running. Press Ctrl+C to exit.");

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
