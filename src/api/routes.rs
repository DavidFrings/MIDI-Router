use crate::{
    api::auth::Lockout,
    router::{
        midi_handler::MidiHandler,
        output_connection::OutputConnection,
        remote::{GRID_NOTES, RemoteHub, RemoteInput},
    },
};
use actix_files::Files;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use log::error;
use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};
use wmidi::Channel;

pub struct ApiState {
    pub hub: Arc<RemoteHub>,
    pub pin: String,
    pub lockout: Lockout,
}

type Reply = Result<HttpResponse, HttpResponse>;

/// 429 while locked, 401 (+ counted fail) on wrong/missing PIN.
pub fn authorize(
    req: &HttpRequest,
    state: &ApiState,
    given: Option<&str>,
) -> Result<(), HttpResponse> {
    let ip = req.peer_addr().map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |a| a.ip());
    let now = Instant::now();
    // check() first: it is the only place an expired lock gets cleared.
    if !state.lockout.check(ip, now) {
        return Err(HttpResponse::TooManyRequests().finish());
    }
    if given != Some(state.pin.as_str()) {
        state.lockout.fail(ip, now);
        return Err(HttpResponse::Unauthorized().finish());
    }
    state.lockout.success(ip);
    Ok(())
}

pub fn configure(
    state: web::Data<ApiState>,
    web_root: Option<PathBuf>,
) -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg| {
        cfg.app_data(state)
            .service(health)
            .service(page_state)
            .service(press)
            .service(fader);
        // Files matches every path, so it goes last.
        match web_root {
            Some(root) => cfg.service(Files::new("/", root).index_file("index.html")),
            None => cfg.route("/", web::get().to(missing_web_root)),
        };
    }
}

fn auth(req: &HttpRequest, state: &ApiState) -> Result<(), HttpResponse> {
    let given = req.headers().get("X-Pin").and_then(|v| v.to_str().ok());
    authorize(req, state, given)
}

fn bank(page: u8) -> Result<Channel, HttpResponse> {
    match page {
        1..=9 => Channel::from_index(page - 1).map_err(|_| HttpResponse::BadRequest().finish()),
        _ => Err(HttpResponse::BadRequest().finish()),
    }
}

/// Fresh handles per request (they go stale on router restart); global lock order.
fn with_router<T>(
    state: &ApiState,
    f: impl FnOnce(&mut MidiHandler, &mut OutputConnection, &mut OutputConnection) -> anyhow::Result<T>,
) -> Result<T, HttpResponse> {
    let h = state.hub.handles().ok_or_else(|| HttpResponse::ServiceUnavailable().finish())?;
    let mut handler = h.handler.lock().unwrap();
    let mut to_controller = h.to_controller.lock().unwrap();
    let mut to_software = h.to_software.lock().unwrap();
    f(&mut handler, &mut to_controller, &mut to_software).map_err(|err| {
        error!(target: "api", "Remote input failed: {:#}", err);
        HttpResponse::InternalServerError().finish()
    })
}

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[derive(Deserialize)]
struct PageQuery {
    page: u8,
}

#[get("/api/state")]
async fn page_state(req: HttpRequest, state: web::Data<ApiState>, q: web::Query<PageQuery>) -> HttpResponse {
    (|| -> Reply {
        auth(&req, &state)?;
        let bank = bank(q.page)?;
        let page = with_router(&state, |h, _, _| Ok(h.snapshot(bank)))?;
        Ok(HttpResponse::Ok().json(page))
    })()
    .unwrap_or_else(|r| r)
}

#[derive(Deserialize)]
struct PressBody {
    page: u8,
    note: u8,
    pressed: bool,
}

#[post("/api/press")]
async fn press(req: HttpRequest, state: web::Data<ApiState>, body: web::Json<PressBody>) -> HttpResponse {
    (|| -> Reply {
        auth(&req, &state)?;
        let bank = bank(body.page)?;
        if !GRID_NOTES.contains(&body.note) {
            return Err(HttpResponse::BadRequest().finish());
        }
        let input = RemoteInput::Press { note: body.note, pressed: body.pressed };
        with_router(&state, |h, c, s| h.handle_remote_input(input, bank, c, s))?;
        Ok(HttpResponse::NoContent().finish())
    })()
    .unwrap_or_else(|r| r)
}

#[derive(Deserialize)]
struct FaderBody {
    page: u8,
    index: usize,
    value: u8,
}

#[post("/api/fader")]
async fn fader(req: HttpRequest, state: web::Data<ApiState>, body: web::Json<FaderBody>) -> HttpResponse {
    (|| -> Reply {
        auth(&req, &state)?;
        let bank = bank(body.page)?;
        if body.index > 8 || body.value > 127 {
            return Err(HttpResponse::BadRequest().finish());
        }
        let input = RemoteInput::Fader { index: body.index, value: body.value };
        with_router(&state, |h, c, s| h.handle_remote_input(input, bank, c, s))?;
        Ok(HttpResponse::NoContent().finish())
    })()
    .unwrap_or_else(|r| r)
}

async fn missing_web_root() -> HttpResponse {
    HttpResponse::InternalServerError()
        .content_type("text/html; charset=utf-8")
        .body("<!doctype html><title>MIDI Router</title><h1>web-remote files not found</h1>\
               <p>Reinstall the MIDI Router or put the <code>web-remote</code> folder next to the executable.</p>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{mapping_config::MappingConfig, remote::RouterHandles};
    use actix_web::{App, http::StatusCode, test};
    use serde_json::{Value, json};
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };
    use wmidi::{Channel, MidiMessage::NoteOn, Note, Velocity};

    const PIN: &str = "4711";

    fn handles() -> RouterHandles {
        let config: MappingConfig = toml::from_str(
            "toggle_notes = [5]\nmomentary_notes = [82]\nnote_map = []\ncontrol_map = []",
        )
        .unwrap();
        RouterHandles {
            handler: Arc::new(Mutex::new(MidiHandler::new(config))),
            to_controller: Arc::new(Mutex::new(OutputConnection::new())),
            to_software: Arc::new(Mutex::new(OutputConnection::new())),
        }
    }

    fn state(attached: Option<RouterHandles>) -> web::Data<ApiState> {
        let hub = RemoteHub::new();
        if let Some(h) = attached {
            hub.attach(h);
        }
        web::Data::new(ApiState { hub, pin: PIN.into(), lockout: Lockout::default() })
    }

    macro_rules! app {
        ($state:expr) => {
            app!($state, None)
        };
        ($state:expr, $root:expr) => {
            test::init_service(App::new().configure(configure($state.clone(), $root))).await
        };
    }

    fn get(uri: &str, pin: Option<&str>) -> test::TestRequest {
        let req = test::TestRequest::get().uri(uri);
        match pin {
            Some(p) => req.insert_header(("X-Pin", p)),
            None => req,
        }
    }

    fn post(uri: &str, body: Value) -> test::TestRequest {
        test::TestRequest::post().uri(uri).insert_header(("X-Pin", PIN)).set_json(body)
    }

    #[actix_web::test]
    async fn health_is_open() {
        let app = app!(state(None));
        let res = test::call_service(&app, get("/health", None).to_request()).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn state_without_pin_is_401() {
        let app = app!(state(Some(handles())));
        let res = test::call_service(&app, get("/api/state?page=1", None).to_request()).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn state_with_pin_returns_page() {
        let app = app!(state(Some(handles())));
        let res = test::call_service(&app, get("/api/state?page=3", Some(PIN)).to_request()).await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = test::read_body_json(res).await;
        assert_eq!(body["page"], 3);
        assert_eq!(body["pads"].as_array().unwrap().len(), 45);
        assert_eq!(body["faders"].as_array().unwrap().len(), 9);
    }

    #[actix_web::test]
    async fn press_sends_to_software_on_page_channel() {
        let h = handles();
        let app = app!(state(Some(h.clone())));
        let res = test::call_service(
            &app,
            post("/api/press", json!({"page": 3, "note": 5, "pressed": true})).to_request(),
        )
        .await;
        assert!(res.status().is_success(), "{}", res.status());
        let sent = h.to_software.lock().unwrap().sent.clone();
        assert!(sent.iter().any(|m| m[0] == 0x92 && m[1] == 5), "{sent:?}");
    }

    #[actix_web::test]
    async fn fader_sends_to_software_on_page_channel() {
        let h = handles();
        let app = app!(state(Some(h.clone())));
        let res = test::call_service(
            &app,
            post("/api/fader", json!({"page": 2, "index": 0, "value": 64})).to_request(),
        )
        .await;
        assert!(res.status().is_success(), "{}", res.status());
        let sent = h.to_software.lock().unwrap().sent.clone();
        assert!(sent.iter().any(|m| m[0] == 0xB1 && m[2] == 64), "{sent:?}");
    }

    #[actix_web::test]
    async fn bad_input_is_400() {
        let app = app!(state(Some(handles())));
        let cases = [
            post("/api/press", json!({"page": 0, "note": 5, "pressed": true})),
            post("/api/press", json!({"page": 10, "note": 5, "pressed": true})),
            post("/api/press", json!({"page": 1, "note": 50, "pressed": true})),
            post("/api/fader", json!({"page": 1, "index": 9, "value": 1})),
            post("/api/fader", json!({"page": 1, "index": 0, "value": 128})),
            get("/api/state?page=0", Some(PIN)),
            get("/api/state?page=10", Some(PIN)),
        ];
        for req in cases {
            let res = test::call_service(&app, req.to_request()).await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[actix_web::test]
    async fn sixth_attempt_after_five_wrong_pins_is_429() {
        let app = app!(state(Some(handles())));
        for _ in 0..5 {
            let res = test::call_service(&app, get("/api/state?page=1", Some("0000")).to_request()).await;
            assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        }
        let res = test::call_service(&app, get("/api/state?page=1", Some(PIN)).to_request()).await;
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[actix_web::test]
    async fn detached_router_is_503() {
        let app = app!(state(None));
        let res = test::call_service(&app, get("/api/state?page=1", Some(PIN)).to_request()).await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let res = test::call_service(
            &app,
            post("/api/press", json!({"page": 1, "note": 5, "pressed": true})).to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[actix_web::test]
    async fn static_index_served_when_root_exists() {
        let root = std::env::temp_dir().join(format!("web-remote-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<h1>remote</h1>").unwrap();

        let app = app!(state(None), Some(root.clone()));
        let res = test::call_service(&app, get("/", None).to_request()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(test::read_body(res).await, "<h1>remote</h1>");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[actix_web::test]
    async fn missing_root_shows_error_page() {
        let app = app!(state(None));
        let res = test::call_service(&app, get("/", None).to_request()).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = test::read_body(res).await;
        assert!(String::from_utf8_lossy(&body).contains("web-remote files not found"));
    }

    #[actix_web::test]
    async fn press_does_not_deadlock_with_concurrent_software_feedback() {
        let h = handles();
        let stop = Arc::new(AtomicBool::new(false));

        // Same lock order as input_connection.rs (SoftwareMessage).
        let feedback = {
            let (h, stop) = (h.clone(), stop.clone());
            thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    {
                        let mut handler = h.handler.lock().unwrap();
                        thread::sleep(Duration::from_micros(50)); // widen the race window
                        let mut ctrl = h.to_controller.lock().unwrap();
                        let msg = NoteOn(Channel::Ch1, Note::from_u8_lossy(5), Velocity::from_u8_lossy(1));
                        handler.handle_software_msg(msg, &mut ctrl).unwrap();
                    }
                    // ~10k msg/s, far above real MIDI; a hot spin would starve the unfair std Mutex.
                    thread::sleep(Duration::from_micros(100));
                }
            })
        };

        // Own thread + runtime: a deadlocked std Mutex cannot be cut short by an async timeout.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            actix_web::rt::System::new().block_on(async {
                let app = app!(state(Some(h)));
                for i in 0..200 {
                    let body = json!({"page": 1, "note": 5, "pressed": i % 2 == 0});
                    let res = test::call_service(&app, post("/api/press", body).to_request()).await;
                    assert!(res.status().is_success());
                }
            });
            done_tx.send(()).unwrap();
        });

        let done = done_rx.recv_timeout(Duration::from_secs(5));
        stop.store(true, Ordering::SeqCst);
        assert!(done.is_ok(), "200 presses did not finish within 5 s"); // before join: a deadlocked thread never joins
        feedback.join().unwrap();
    }
}
