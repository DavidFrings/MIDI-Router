mod router;
mod tui;

use crate::tui::App;
use anyhow::Result;
use clap::Parser;
use log::{LevelFilter, Metadata, Record, debug, error, info};
use reqwest::Client;
use router::midi_connection::MidiRouter;
use std::{
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long, default_value = "APC40 mkII")]
    controller_name: String,

    #[clap(short, long, default_value = "Daslight")]
    software_name: String,
}

struct ForwardLogger {
    tx: Mutex<Sender<String>>,
}

impl log::Log for ForwardLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        let msg = format!("{}: {}", record.level(), record.args());
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(msg);
        }
    }
    fn flush(&self) {}
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

fn router_thread(
    restart: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    controller: String,
    software: String,
) {
    thread::spawn(move || {
        loop {
            if exit.load(Ordering::SeqCst) {
                info!("Router thread exiting");
                break;
            }

            debug!("Starting MIDIRouter...");
            let mut router = MidiRouter::new();
            match MidiRouter::connect(&mut router, &controller, &software) {
                Ok(_) => info!("Started MIDIRouter..."),
                Err(err) => error!("MIDIRouter failed: {}", err),
            }

            while !restart.load(Ordering::SeqCst) && !exit.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(200));
            }

            if exit.load(Ordering::SeqCst) {
                info!("Router thread exiting after wait");
                break;
            }

            if restart.load(Ordering::SeqCst) {
                restart.store(false, Ordering::SeqCst);
                debug!("Router restart requested, reconnecting...");
                continue;
            }
        }
    });
}

fn tui_thread(
    restart: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    log: Receiver<String>,
    controller: String,
    software: String,
) -> Result<()> {
    let mut app = App::new(controller, software, exit.clone(), restart, log);

    let handle = thread::spawn(move || {
        let mut terminal = ratatui::init();
        let res = app.run(&mut terminal);
        ratatui::restore();
        res
    });

    let res = handle.join().expect("TUI thread panicked");
    if let Err(err) = res {
        return Err(err);
    }

    exit.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(200));

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging for TUI
    let (log_tx, log_rx) = mpsc::channel::<String>();
    let forward = ForwardLogger {
        tx: Mutex::new(log_tx.clone()),
    };

    log::set_boxed_logger(Box::new(forward))?;
    log::set_max_level(LevelFilter::Info);

    // Check for updates
    info!("Checking for updates...");
    update().await.expect("Failed to check for updates"); // Disable for dev!

    let args = Args::parse();
    let restart_flag = Arc::new(AtomicBool::new(false));
    let exit_flag = Arc::new(AtomicBool::new(false));

    router_thread(
        restart_flag.clone(),
        exit_flag.clone(),
        args.controller_name.clone(),
        args.software_name.clone(),
    );
    tui_thread(
        restart_flag.clone(),
        exit_flag.clone(),
        log_rx,
        args.controller_name.clone(),
        args.software_name.clone(),
    )?;

    Ok(())
}
