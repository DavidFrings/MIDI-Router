use anyhow::Result;
use env_logger::Env;
use log::info;
use reqwest::Client;
use std::{env, fs, process::Command, thread, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let download_url = &args[1];

    // Wait until App is closed
    thread::sleep(Duration::from_secs(2));

    let client = Client::new();
    let res = client.get(download_url).send().await?.bytes().await?;

    let exe_path = "midi-router.exe";
    fs::write(&exe_path, &res)?;

    Command::new(exe_path).spawn()?;

    info!("Updated router!");
    Ok(())
}
