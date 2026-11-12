use crate::router::{
    fader_state::FADER_COUNT, midi_handler::MidiHandler, output_connection::OutputConnection,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Pushed to tablets. `page` is 1-based (`bank.index() + 1`).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RemoteEvent {
    Pad {
        page: u8,
        note: u8,
        on: bool,
        color: Option<u8>,
    },
    Fader { page: u8, index: usize, value: u8 },
    Resync,
}

pub enum RemoteInput {
    Press { note: u8, pressed: bool },
    Fader { index: usize, value: u8 },
}

#[derive(Serialize)]
pub struct PadState {
    pub note: u8,
    pub on: bool,
    pub color: Option<u8>,
}

#[derive(Serialize)]
pub struct PageState {
    pub page: u8,
    pub pads: Vec<PadState>,
    pub faders: [Option<u8>; FADER_COUNT],
}

/// shortcut: APC40 mkII layout only, make it config if another controller needs the remote.
pub const GRID_NOTES: [u8; 45] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 82, 83, 84, 85, 86,
];

#[derive(Clone)]
pub struct RouterHandles {
    pub handler: Arc<Mutex<MidiHandler>>,
    pub to_controller: Arc<Mutex<OutputConnection>>,
    pub to_software: Arc<Mutex<OutputConnection>>,
}

/// Bridge between the router thread (attach/detach per connection) and the web API.
pub struct RemoteHub {
    router: Mutex<Option<RouterHandles>>,
    pub events: broadcast::Sender<RemoteEvent>,
}

impl RemoteHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            router: Mutex::new(None),
            events: broadcast::channel(256).0,
        })
    }

    pub fn attach(&self, handles: RouterHandles) {
        *self.router.lock().unwrap() = Some(handles);
        let _ = self.events.send(RemoteEvent::Resync); // no subscribers is normal
    }

    pub fn detach(&self) {
        *self.router.lock().unwrap() = None;
    }

    pub fn handles(&self) -> Option<RouterHandles> {
        self.router.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::mapping_config::MappingConfig;

    fn handles() -> RouterHandles {
        let config: MappingConfig =
            toml::from_str("toggle_notes = []\nmomentary_notes = []\nnote_map = []\ncontrol_map = []")
                .unwrap();
        RouterHandles {
            handler: Arc::new(Mutex::new(MidiHandler::new(config))),
            to_controller: Arc::new(Mutex::new(OutputConnection::new())),
            to_software: Arc::new(Mutex::new(OutputConnection::new())),
        }
    }

    #[test]
    fn attach_sends_resync() {
        let hub = RemoteHub::new();
        let mut rx = hub.events.subscribe();

        hub.attach(handles());

        assert!(hub.handles().is_some());
        assert_eq!(rx.try_recv().unwrap(), RemoteEvent::Resync);
    }

    #[test]
    fn detach_clears_handles() {
        let hub = RemoteHub::new();
        hub.attach(handles());

        hub.detach();

        assert!(hub.handles().is_none());
    }
}
