mod router;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::{info};
use std::{thread, time::Duration};
use wmidi::Velocity;
use router::midi_connection::MidiRouter;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long, default_value = "APC40 mkII")]
    controller_name: String,

    #[clap(short, long, default_value = "Daslight")]
    software_name: String
}

const LED_ON: Velocity = Velocity::from_u8_lossy(122); // Green

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let args = Args::parse();

    info!("Controller MIDI port: {}", args.controller_name);
    info!("Software MIDI port: {}", args.software_name);
    //let mut router = midi_router::MidiRouter::new();
    let mut router = MidiRouter::new();
    info!("Connecting to MIDI ports...");
    //midi_router::MidiRouter::connect(&mut router, "APC40 mkII", "to_Daslight", "from_Daslight")?;
    MidiRouter::connect(&mut router, &args.controller_name, &args.software_name)?;
    info!("Connected successfully!");

    info!("Router is running. Press Ctrl+C to exit.");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}