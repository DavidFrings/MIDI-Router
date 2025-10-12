use crate::{
    api::test::test,
    router::{mapping_config::MappingConfig, midi_connection::MidiRouter},
    utils::{config::ApiConfig, tui::App},
};
use actix_web::HttpServer;
use anyhow::Result;
use log::{debug, error, info};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    thread,
    time::Duration,
};
use tokio::runtime::Runtime;

pub(crate) fn tui_thread(
    restart: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    log_router: Receiver<String>,
    log_api: Receiver<String>,
    controller: String,
    software: String,
) -> Result<()> {
    let mut app = App::new(
        controller,
        software,
        exit.clone(),
        restart,
        log_router,
        log_api,
    );

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

pub(crate) fn api_thread(exit: Arc<AtomicBool>, config: ApiConfig) {
    thread::spawn(move || {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            info!(target: "api",
                "Starting REST api on {}:{}",
                config.bind_address, config.port
            );

            let server = HttpServer::new(|| {
                actix_web::App::new()
                    .wrap(actix_web::middleware::Logger::default().exclude("/health"))
                    .service(test)
            })
            .bind((config.bind_address.clone(), config.port))
            .expect(&format!(
                "Failed to bind api to {}:{}",
                config.bind_address, config.port
            ))
            .disable_signals()
            .run();

            let handle = server.handle();

            let exit_clone = exit.clone();
            tokio::spawn(async move {
                while !exit_clone.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                handle.stop(true).await;
                info!(target: "api", "REST api stopped");
            });

            if let Err(err) = server.await {
                error!(target: "api", "REST api error: {}", err);
            }
        });
    });
}

pub(crate) fn router_thread(
    restart: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    config: MappingConfig,
    controller: String,
    software: String,
) {
    thread::spawn(move || {
        while should_continue(&exit) {
            router_iteration(&restart, &exit, &config, &controller, &software);

            if should_restart(&restart, &exit) {
                restart.store(false, Ordering::SeqCst);
                debug!("Router restart requested, reconnecting...");
            }
        }
        info!("Router thread exiting");
    });
}

fn should_continue(exit: &Arc<AtomicBool>) -> bool {
    !exit.load(Ordering::SeqCst)
}

fn should_restart(restart: &Arc<AtomicBool>, exit: &Arc<AtomicBool>) -> bool {
    restart.load(Ordering::SeqCst) && !exit.load(Ordering::SeqCst)
}

fn router_iteration(
    restart: &Arc<AtomicBool>,
    exit: &Arc<AtomicBool>,
    config: &MappingConfig,
    controller: &str,
    software: &str,
) {
    debug!("Starting MIDIRouter...");
    let mut router = MidiRouter::new(config.clone());

    match MidiRouter::connect(&mut router, controller, software) {
        Ok(_) => info!("Started MIDIRouter..."),
        Err(err) => error!("MIDIRouter failed: {}", err),
    }

    while !restart.load(Ordering::SeqCst) && !exit.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(200));
    }
}
