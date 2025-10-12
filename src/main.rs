mod api;
mod router;
mod utils;

use crate::utils::{
    config::Config,
    helper::{ForwardLogger, update},
    threads::{api_thread, router_thread, tui_thread},
};
use anyhow::Result;
use log::{LevelFilter, info};
use std::sync::{
    Arc, Mutex,
    atomic::AtomicBool,
    mpsc::{Receiver, channel},
};

fn logging() -> Result<(Receiver<String>, Receiver<String>)> {
    let (log_tx_router, log_rx_router) = channel::<String>();
    let (log_tx_api, log_rx_api) = channel::<String>();

    let forward = ForwardLogger {
        tx_router: Mutex::new(log_tx_router),
        tx_api: Mutex::new(log_tx_api.clone()),
    };

    log::set_boxed_logger(Box::new(forward))?;
    log::set_max_level(LevelFilter::Info);

    Ok((log_rx_router, log_rx_api))
}

async fn check_update(config: &Config) -> Result<()> {
    info!("Checking for updates...");

    if config.dev {
        info!("Skipping update check");
        return Ok(());
    }

    update().await.expect("Failed to check for updates");
    Ok(())
}

fn init_threads(config: &Config, logs: (Receiver<String>, Receiver<String>)) -> Result<()> {
    let restart = Arc::new(AtomicBool::new(false));
    let exit = Arc::new(AtomicBool::new(false));

    router_thread(
        restart.clone(),
        exit.clone(),
        config.maps.clone(),
        config.router.controller_name.clone(),
        config.router.software_name.clone(),
    );
    api_thread(exit.clone(), config.api.clone());
    tui_thread(
        restart.clone(),
        exit.clone(),
        logs.0,
        logs.1,
        config.router.controller_name.clone(),
        config.router.software_name.clone(),
    )?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let logs = logging()?;
    let config = &Config::new()?;

    check_update(config).await?;
    init_threads(config, logs)?;

    Ok(())
}
