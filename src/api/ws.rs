use crate::api::routes::{ApiState, authorize};
use actix_web::{HttpRequest, HttpResponse, get, rt, web};
use actix_ws::Message;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

#[derive(Deserialize)]
struct WsQuery {
    pin: Option<String>,
}

/// Browsers cannot set WS headers, so the PIN comes from `?pin=`.
#[get("/ws")]
async fn ws(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<ApiState>,
    q: web::Query<WsQuery>,
) -> actix_web::Result<HttpResponse> {
    if let Err(res) = authorize(&req, &state, q.pin.as_deref()) {
        return Ok(res);
    }
    // Subscribe before the upgrade completes so no event falls in the gap.
    let mut events = state.hub.events.subscribe();
    let (res, mut session, mut stream) = actix_ws::handle(&req, body)?;

    rt::spawn(async move {
        loop {
            tokio::select! {
                ev = events.recv() => {
                    let text = match ev {
                        Ok(ev) => serde_json::to_string(&ev).unwrap_or_default(),
                        Err(RecvError::Lagged(_)) => r#"{"type":"resync"}"#.to_owned(),
                        Err(RecvError::Closed) => break,
                    };
                    if session.text(text).await.is_err() {
                        break;
                    }
                }
                msg = stream.recv() => match msg {
                    Some(Ok(Message::Ping(b))) => {
                        if session.pong(&b).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(reason))) => {
                        let _ = session.close(reason).await;
                        return;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                },
            }
        }
        let _ = session.close(None).await;
    });

    Ok(res)
}

#[cfg(test)]
mod tests {
    use crate::api::{
        auth::Lockout,
        routes::{ApiState, configure},
    };
    use crate::router::{
        mapping_config::MappingConfig,
        midi_handler::MidiHandler,
        output_connection::OutputConnection,
        remote::{RemoteHub, RouterHandles},
    };
    use actix_web::web;
    use awc::ws::Frame;
    use futures_util::StreamExt;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const PIN: &str = "4711";

    fn handles(hub: &RemoteHub) -> RouterHandles {
        let config: MappingConfig = toml::from_str(
            "toggle_notes = [5]\nmomentary_notes = [82]\nnote_map = []\ncontrol_map = []",
        )
        .unwrap();
        let mut handler = MidiHandler::new(config);
        handler.set_event_sender(hub.events.clone());
        RouterHandles {
            handler: Arc::new(Mutex::new(handler)),
            to_controller: Arc::new(Mutex::new(OutputConnection::new())),
            to_software: Arc::new(Mutex::new(OutputConnection::new())),
        }
    }

    fn server() -> (actix_test::TestServer, Arc<RemoteHub>) {
        let hub = RemoteHub::new();
        let state = web::Data::new(ApiState { hub: hub.clone(), pin: PIN.into(), lockout: Lockout::default() });
        let srv = actix_test::start(move || actix_web::App::new().configure(configure(state.clone(), None)));
        (srv, hub)
    }

    async fn next_json<S>(ws: &mut S) -> Value
    where
        S: futures_util::Stream<Item = Result<Frame, awc::error::WsProtocolError>> + Unpin,
    {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await
                .expect("no frame in 3 s")
                .expect("stream ended")
                .unwrap();
            if let Frame::Text(t) = frame {
                return serde_json::from_slice(&t).unwrap();
            }
        }
    }

    #[actix_web::test]
    async fn ws_wrong_pin_is_rejected() {
        let (mut srv, _) = server();
        for path in ["/ws?pin=0000", "/ws"] {
            let err = srv.ws_at(path).await.err().expect("handshake must fail");
            assert!(format!("{err:?}").contains("401"), "{err:?}");
        }
    }

    #[actix_web::test]
    async fn ws_receives_pad_event_after_press() {
        let (mut srv, hub) = server();
        hub.attach(handles(&hub));
        let mut ws = srv.ws_at(&format!("/ws?pin={PIN}")).await.unwrap();

        let res = srv
            .post("/api/press")
            .insert_header(("X-Pin", PIN))
            .send_json(&json!({"page": 1, "note": 5, "pressed": true}))
            .await
            .unwrap();
        assert_eq!(res.status(), 204);

        let ev = next_json(&mut ws).await;
        assert_eq!(ev["type"], "pad");
        assert_eq!(ev["page"], 1);
    }

    #[actix_web::test]
    async fn ws_receives_resync_on_attach() {
        let (mut srv, hub) = server();
        let mut ws = srv.ws_at(&format!("/ws?pin={PIN}")).await.unwrap();
        hub.attach(handles(&hub));
        assert_eq!(next_json(&mut ws).await, json!({"type": "resync"}));
    }
}
