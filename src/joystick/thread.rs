use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use log::error;
use wmidi::{Channel, ControlFunction, MidiMessage, U7};

use super::{
    device::JoystickSource,
    state::{JoystickSnapshot, JoystickState},
};
use crate::router::remote::{RemoteEvent, RemoteHub};

/// One poll/tick cycle. `last` is the snapshot last broadcast to tablets.
pub fn step(
    state: &Mutex<JoystickState>,
    source: &mut dyn JoystickSource,
    hub: &RemoteHub,
    last: &mut Option<JoystickSnapshot>,
    dt: f32,
    now: Instant,
) {
    let inputs = source.poll();
    let handles = hub.handles();
    // Joystick lock is released before to_software is taken (lock order: handler, to_controller, to_software).
    let (sends, channel, snap) = {
        let mut s = state.lock().unwrap();
        for input in inputs {
            s.input(input);
        }
        let sends = if handles.is_some() {
            s.tick(dt, now)
        } else {
            Vec::new()
        };
        (sends, s.channel(), s.snapshot())
    };

    if let Some(handles) = handles
        && !sends.is_empty()
    {
        let channel = Channel::from_index(channel - 1).expect("channel validated in config");
        let mut out = handles.to_software.lock().unwrap();
        for (cc, value) in sends {
            let msg = MidiMessage::ControlChange(
                channel,
                ControlFunction::from(U7::from_u8_lossy(cc)),
                U7::from_u8_lossy(value),
            );
            if let Err(err) = out.send(msg) {
                error!("Joystick send failed: {err}");
            }
        }
    }

    if last.as_ref() != Some(&snap) {
        let _ = hub.events.send(RemoteEvent::Joystick(snap.clone())); // no subscribers is normal
        *last = Some(snap);
    }
}

pub fn joystick_thread(
    exit: Arc<AtomicBool>,
    hub: Arc<RemoteHub>,
    state: Arc<Mutex<JoystickState>>,
    mut source: Box<dyn JoystickSource + Send>,
) {
    thread::spawn(move || {
        let mut last = None;
        let mut prev = Instant::now();
        while !exit.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
            let now = Instant::now();
            let dt = now.duration_since(prev).as_secs_f32();
            prev = now;
            step(&state, source.as_mut(), &hub, &mut last, dt, now);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joystick::state::{Axis, JoyInput};
    use crate::router::{
        mapping_config::MappingConfig, midi_handler::MidiHandler,
        output_connection::OutputConnection, remote::RouterHandles,
    };

    struct Script(Vec<Vec<JoyInput>>);

    impl JoystickSource for Script {
        fn poll(&mut self) -> Vec<JoyInput> {
            if self.0.is_empty() {
                Vec::new()
            } else {
                self.0.remove(0)
            }
        }
    }

    fn state() -> Mutex<JoystickState> {
        Mutex::new(JoystickState::new(
            toml::from_str(
                "channel = 16\nbanks = [{pan = 20, tilt = 21}, {pan = 30, tilt = 31}, {pan = 40, tilt = 41}]",
            )
            .unwrap(),
        ))
    }

    fn attached() -> (Arc<RemoteHub>, RouterHandles) {
        let config: MappingConfig = toml::from_str(
            "toggle_notes = []\nmomentary_notes = []\nnote_map = []\ncontrol_map = []",
        )
        .unwrap();
        let handles = RouterHandles {
            handler: Arc::new(Mutex::new(MidiHandler::new(config))),
            to_controller: Arc::new(Mutex::new(OutputConnection::new())),
            to_software: Arc::new(Mutex::new(OutputConnection::new())),
        };
        let hub = RemoteHub::new();
        hub.attach(handles.clone());
        (hub, handles)
    }

    fn push_right() -> Vec<JoyInput> {
        vec![
            JoyInput::Button {
                index: 1,
                pressed: true,
            },
            JoyInput::Axis {
                axis: Axis::X,
                value: 1.0,
            },
        ]
    }

    fn joystick_events(
        rx: &mut tokio::sync::broadcast::Receiver<RemoteEvent>,
    ) -> Vec<JoystickSnapshot> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|e| match e {
                RemoteEvent::Joystick(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn step_sends_cc_on_joystick_channel() {
        let (hub, handles) = attached();
        let state = state();

        step(
            &state,
            &mut Script(vec![push_right()]),
            &hub,
            &mut None,
            0.1,
            Instant::now(),
        );

        let sent = handles.to_software.lock().unwrap().sent.clone();
        assert!(sent.contains(&vec![0xBF, 20, 70]), "{sent:?}");
    }

    #[test]
    fn step_broadcasts_snapshot_on_change() {
        let (hub, _handles) = attached();
        let state = state();
        let mut rx = hub.events.subscribe();
        let mut last = None;
        let bank2 = vec![JoyInput::Button {
            index: 6,
            pressed: true,
        }];
        let mut source = Script(vec![bank2]);

        step(&state, &mut source, &hub, &mut last, 0.01, Instant::now());
        assert_eq!(joystick_events(&mut rx).last().unwrap().bank, 2);

        step(&state, &mut source, &hub, &mut last, 0.01, Instant::now());
        assert!(joystick_events(&mut rx).is_empty());
    }

    #[test]
    fn step_without_router_does_not_move() {
        let hub = RemoteHub::new();
        let state = state();
        let mut input = push_right();
        input.push(JoyInput::Button {
            index: 6,
            pressed: true,
        });

        step(
            &state,
            &mut Script(vec![input]),
            &hub,
            &mut None,
            0.1,
            Instant::now(),
        );

        let snap = state.lock().unwrap().snapshot();
        assert_eq!(snap.values, [Some(64), Some(64), None, None]);
        assert_eq!(snap.bank, 2);
    }
}
