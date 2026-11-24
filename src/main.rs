mod api;
mod joystick;
mod router;
mod utils;

use crate::api::auth::valid_pin;
use crate::joystick::{device, state::JoystickState, thread::joystick_thread};
use crate::router::remote::RemoteHub;
use crate::utils::{
    config::Config,
    helper::{ForwardLogger, lan_host, update, web_remote_line, web_url},
    threads::{api_thread, router_thread, tui_thread},
};
use anyhow::Result;
use log::{LevelFilter, info, warn};
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

    // Never block the router: offline venues and GitHub rate limits are normal.
    if let Err(err) = update().await {
        warn!("Update check failed, starting anyway: {:#}", err);
    }
    Ok(())
}

fn init_threads(config: &Config, logs: (Receiver<String>, Receiver<String>)) -> Result<()> {
    let restart = Arc::new(AtomicBool::new(false));
    let exit = Arc::new(AtomicBool::new(false));
    let hub = RemoteHub::new();
    if let Some(cfg) = &config.joystick {
        let state = Arc::new(Mutex::new(JoystickState::new(cfg.clone())));
        let _ = hub.joystick.set(state.clone()); // hub is fresh, cannot be set yet
        joystick_thread(exit.clone(), hub.clone(), state, device::open(&cfg.device));
    }

    router_thread(
        restart.clone(),
        exit.clone(),
        config.maps.clone(),
        config.router.controller_name.clone(),
        config.router.software_name.clone(),
        hub.clone(),
    );
    let (web, local) = if config.api.enabled && valid_pin(&config.api.pin) {
        api_thread(exit.clone(), config.api.clone(), hub);
        let port = config.api.port;
        let lan = lan_host(&config.api.bind_address).map(|h| web_url(&h, port));
        (
            web_remote_line(true, lan.as_deref(), port),
            Some(web_url("localhost", port)),
        )
    } else {
        warn!(target: "api", "Web remote off: set pin in config.toml (digits only, at least 4)");
        (web_remote_line(false, None, 0), None)
    };
    tui_thread(
        restart.clone(),
        exit.clone(),
        logs.0,
        logs.1,
        config.router.controller_name.clone(),
        config.router.software_name.clone(),
        web,
        local,
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
