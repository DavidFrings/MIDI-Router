use std::time::{Duration, Instant};

use serde::Serialize;

use super::config::JoystickConfig;

/// A CC we sent is echoed back by the host; ignore feedback for this long after sending.
const ECHO_GUARD: Duration = Duration::from_millis(200);
/// Cap on a single tick so a stalled thread cannot make the head jump.
const MAX_DT: f32 = 0.1;
const START: f32 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    X,
    Y,
}

#[allow(dead_code)] // constructed by the device backend (Task 7)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JoyInput {
    /// `index` is 1-based (manual numbering).
    Button {
        index: u8,
        pressed: bool,
    },
    /// -1/0/1, x: -1 = left.
    Hat {
        x: i8,
        y: i8,
    },
    /// -1..1; X +1 = right, Y +1 = forward.
    Axis {
        axis: Axis,
        value: f32,
    },
    Connected(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JoystickSnapshot {
    pub connected: bool,
    /// 1-based.
    pub bank: u8,
    pub banks: u8,
    pub fine: bool,
    /// `[pan, tilt, pan_fine, tilt_fine]` of the current bank.
    pub values: [Option<u8>; 4],
}

struct Bank {
    ccs: [Option<u8>; 4],
    values: [f32; 4],
    fine: bool,
    last_sent: [Option<Instant>; 4],
}

pub struct JoystickState {
    channel: u8,
    speed: f32,
    deadzone: f32,
    curve: f32,
    banks: Vec<Bank>,
    current: usize,
    connected: bool,
    trigger: bool,
    axes: [f32; 2],
    hat_x: i8,
}

impl JoystickState {
    pub fn new(config: JoystickConfig) -> Self {
        let banks = config
            .banks
            .iter()
            .map(|b| Bank {
                ccs: [Some(b.pan), Some(b.tilt), b.pan_fine, b.tilt_fine],
                values: [START; 4],
                fine: false,
                last_sent: [None; 4],
            })
            .collect();
        Self {
            channel: config.channel,
            speed: config.speed,
            deadzone: config.deadzone,
            curve: config.curve,
            banks,
            current: 0,
            connected: false,
            trigger: false,
            axes: [0.0; 2],
            hat_x: 0,
        }
    }

    pub fn channel(&self) -> u8 {
        self.channel
    }

    pub fn input(&mut self, input: JoyInput) {
        match input {
            JoyInput::Button {
                index,
                pressed: true,
            } => match index {
                1 => self.trigger = true,
                2 => {
                    let b = &mut self.banks[self.current];
                    b.fine = !b.fine;
                }
                5..=16 => {
                    let bank = (index - 5) as usize;
                    if bank < self.banks.len() {
                        self.current = bank;
                    }
                }
                _ => {}
            },
            JoyInput::Button {
                index: 1,
                pressed: false,
            } => self.trigger = false,
            JoyInput::Button { .. } => {}
            JoyInput::Hat { x, .. } => {
                let n = self.banks.len();
                if x != 0 && self.hat_x == 0 {
                    self.current = if x > 0 {
                        (self.current + 1) % n
                    } else {
                        (self.current + n - 1) % n
                    };
                }
                self.hat_x = x;
            }
            JoyInput::Axis { axis, value } => self.axes[axis as usize] = value,
            JoyInput::Connected(c) => {
                self.connected = c;
                if !c {
                    self.trigger = false;
                    self.axes = [0.0; 2];
                    self.hat_x = 0;
                }
            }
        }
    }

    /// Units per second at deflection `d`.
    fn rate(&self, d: f32) -> f32 {
        let a = d.abs();
        if a <= self.deadzone {
            return 0.0;
        }
        d.signum() * ((a - self.deadzone) / (1.0 - self.deadzone)).powf(self.curve) * 127.0
            / self.speed
    }

    /// Returns `(cc, value)` pairs to send.
    pub fn tick(&mut self, dt: f32, now: Instant) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        if !self.trigger {
            return out;
        }
        let dt = dt.clamp(0.0, MAX_DT);
        let rates = [
            self.rate(self.axes[Axis::X as usize]),
            self.rate(self.axes[Axis::Y as usize]),
        ];
        let bank = &mut self.banks[self.current];
        let base = if bank.fine { 2 } else { 0 };
        for (i, rate) in rates.into_iter().enumerate() {
            let slot = base + i;
            let Some(cc) = bank.ccs[slot] else { continue };
            let old = bank.values[slot];
            let new = (old + rate * dt).clamp(0.0, 127.0);
            bank.values[slot] = new;
            if new.round() != old.round() {
                bank.last_sent[slot] = Some(now);
                out.push((cc, new.round() as u8));
            }
        }
        out
    }

    /// Applies an incoming CC to every bank that has it. True if a stored value changed.
    pub fn feedback(&mut self, cc: u8, value: u8, now: Instant) -> bool {
        let mut changed = false;
        for bank in &mut self.banks {
            for slot in 0..4 {
                if bank.ccs[slot] != Some(cc) {
                    continue;
                }
                if bank.last_sent[slot]
                    .is_some_and(|t| now.saturating_duration_since(t) < ECHO_GUARD)
                {
                    continue;
                }
                // Keep the fractional accumulator when the rounded value already matches.
                if bank.values[slot].round() as u8 != value {
                    bank.values[slot] = value as f32;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn snapshot(&self) -> JoystickSnapshot {
        let b = &self.banks[self.current];
        JoystickSnapshot {
            connected: self.connected,
            bank: self.current as u8 + 1,
            banks: self.banks.len() as u8,
            fine: b.fine,
            values: std::array::from_fn(|i| b.ccs[i].map(|_| b.values[i].round() as u8)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joystick::config::BankConfig;

    fn state() -> JoystickState {
        JoystickState::new(JoystickConfig {
            channel: 16,
            device: String::new(),
            speed: 2.0,
            deadzone: 0.1,
            curve: 2.0,
            banks: vec![
                BankConfig {
                    pan: 20,
                    tilt: 21,
                    pan_fine: Some(22),
                    tilt_fine: Some(23),
                },
                BankConfig {
                    pan: 30,
                    tilt: 31,
                    pan_fine: None,
                    tilt_fine: None,
                },
                BankConfig {
                    pan: 40,
                    tilt: 41,
                    pan_fine: None,
                    tilt_fine: None,
                },
            ],
        })
    }

    fn btn(s: &mut JoystickState, index: u8) {
        s.input(JoyInput::Button {
            index,
            pressed: true,
        });
        s.input(JoyInput::Button {
            index,
            pressed: false,
        });
    }
    fn trigger(s: &mut JoystickState, pressed: bool) {
        s.input(JoyInput::Button { index: 1, pressed });
    }
    fn axis(s: &mut JoystickState, a: Axis, value: f32) {
        s.input(JoyInput::Axis { axis: a, value });
    }
    fn hat(s: &mut JoystickState, x: i8) {
        s.input(JoyInput::Hat { x, y: 0 });
    }
    fn held(x: f32) -> (JoystickState, Instant) {
        let mut s = state();
        trigger(&mut s, true);
        axis(&mut s, Axis::X, x);
        (s, Instant::now())
    }

    #[test]
    fn starts_at_64_on_bank_1() {
        let mut s = state();
        assert_eq!(
            s.snapshot(),
            JoystickSnapshot {
                connected: false,
                bank: 1,
                banks: 3,
                fine: false,
                values: [Some(64); 4]
            }
        );
        btn(&mut s, 6);
        assert_eq!(s.snapshot().values, [Some(64), Some(64), None, None]);
    }

    #[test]
    fn no_move_without_trigger() {
        let mut s = state();
        axis(&mut s, Axis::X, 1.0);
        assert!(s.tick(1.0, Instant::now()).is_empty());
        assert_eq!(s.snapshot().values[0], Some(64));
    }

    #[test]
    fn full_deflection_crosses_range_in_speed() {
        let (mut s, now) = held(1.0);
        let mut out = vec![];
        for _ in 0..10 {
            out.extend(s.tick(0.1, now));
        }
        assert_eq!(s.snapshot().values[0], Some(127));
        assert!(out.contains(&(20, 127)));
        assert_eq!(s.snapshot().values[1], Some(64));
        axis(&mut s, Axis::X, -1.0);
        for _ in 0..20 {
            s.tick(0.1, now);
        }
        assert_eq!(s.snapshot().values[0], Some(0));
    }

    #[test]
    fn y_forward_raises_tilt() {
        let mut s = state();
        trigger(&mut s, true);
        axis(&mut s, Axis::Y, 1.0);
        let out = s.tick(0.1, Instant::now());
        assert!(out.iter().all(|(cc, _)| *cc == 21));
        assert!(!out.is_empty());
        assert!(s.snapshot().values[1].unwrap() > 64);
    }

    #[test]
    fn deadzone_and_curve() {
        let (mut s, now) = held(0.1);
        assert!(s.tick(1.0, now).is_empty());
        axis(&mut s, Axis::X, 0.55);
        for _ in 0..10 {
            s.tick(0.1, now);
        }
        let moved = s.snapshot().values[0].unwrap() as i32 - 64;
        assert!((moved - 16).abs() <= 1, "moved {moved}");
    }

    #[test]
    fn slow_move_accumulates() {
        let (mut s, now) = held(0.19);
        let mut out = vec![];
        for _ in 0..100 {
            out.extend(s.tick(0.01, now));
        }
        assert_eq!(s.snapshot().values[0], Some(65));
        assert!(out.contains(&(20, 65)));
    }

    #[test]
    fn feedback_same_value_keeps_accumulator() {
        let (mut s, t) = held(0.19);
        for _ in 0..70 {
            assert!(s.tick(0.01, t).is_empty());
        }
        assert!(!s.feedback(20, 64, t + Duration::from_millis(300)));
        let mut out = vec![];
        for _ in 0..30 {
            out.extend(s.tick(0.01, t));
        }
        assert!(out.contains(&(20, 65)));
    }

    #[test]
    fn only_changed_values_are_sent() {
        let (mut s, now) = held(0.0);
        assert!(s.tick(0.1, now).is_empty());
        assert!(s.tick(0.1, now).is_empty());
    }

    #[test]
    fn release_trigger_stops() {
        let (mut s, now) = held(1.0);
        assert!(!s.tick(0.1, now).is_empty());
        trigger(&mut s, false);
        assert!(s.tick(0.1, now).is_empty());
    }

    #[test]
    fn fine_toggle_moves_fine_only() {
        let (mut s, now) = held(1.0);
        btn(&mut s, 2);
        assert!(s.snapshot().fine);
        let out = s.tick(0.1, now);
        assert!(out.iter().any(|(cc, _)| *cc == 22));
        assert!(out.iter().all(|(cc, _)| *cc != 20));
        btn(&mut s, 2);
        assert!(!s.snapshot().fine);
    }

    #[test]
    fn fine_without_fine_cc_does_nothing() {
        let (mut s, now) = held(1.0);
        btn(&mut s, 6);
        btn(&mut s, 2);
        assert!(s.snapshot().fine);
        assert!(s.tick(0.1, now).is_empty());
    }

    #[test]
    fn fine_is_per_bank() {
        let mut s = state();
        btn(&mut s, 2);
        btn(&mut s, 6);
        assert!(!s.snapshot().fine);
        btn(&mut s, 5);
        assert!(s.snapshot().fine);
    }

    #[test]
    fn hat_steps_and_wraps() {
        let mut s = state();
        for want in [2, 3, 1] {
            hat(&mut s, 1);
            assert_eq!(s.snapshot().bank, want);
            hat(&mut s, 0);
        }
        hat(&mut s, -1);
        assert_eq!(s.snapshot().bank, 3);
        hat(&mut s, 0);
        hat(&mut s, 1);
        hat(&mut s, 1);
        assert_eq!(s.snapshot().bank, 1);
        s.input(JoyInput::Hat { x: 0, y: 1 });
        assert_eq!(s.snapshot().bank, 1);
    }

    #[test]
    fn base_buttons_select_banks() {
        let mut s = state();
        btn(&mut s, 6);
        assert_eq!(s.snapshot().bank, 2);
        btn(&mut s, 9);
        assert_eq!(s.snapshot().bank, 2);
        for i in [3, 4, 17] {
            btn(&mut s, i);
            assert_eq!(s.snapshot().bank, 2);
        }
    }

    #[test]
    fn banks_remember_values() {
        let (mut s, now) = held(1.0);
        s.tick(0.1, now);
        let v = s.snapshot().values[0];
        assert_ne!(v, Some(64));
        btn(&mut s, 6);
        assert_eq!(s.snapshot().values[0], Some(64));
        btn(&mut s, 5);
        assert_eq!(s.snapshot().values[0], v);
    }

    #[test]
    fn feedback_sets_value() {
        let mut s = state();
        let now = Instant::now();
        assert!(s.feedback(21, 100, now));
        assert_eq!(s.snapshot().values[1], Some(100));
        assert!(!s.feedback(99, 5, now));
        trigger(&mut s, true);
        axis(&mut s, Axis::Y, 1.0);
        s.tick(0.1, now);
        assert!(s.snapshot().values[1].unwrap() > 100);
    }

    #[test]
    fn feedback_echo_ignored() {
        let (mut s, t) = held(1.0);
        assert!(!s.tick(0.1, t).is_empty());
        let v = s.snapshot().values[0];
        assert!(!s.feedback(20, 10, t + Duration::from_millis(100)));
        assert_eq!(s.snapshot().values[0], v);
        assert!(s.feedback(20, 10, t + Duration::from_millis(300)));
        assert_eq!(s.snapshot().values[0], Some(10));
    }

    #[test]
    fn disconnect_stops_movement() {
        let (mut s, now) = held(1.0);
        s.input(JoyInput::Connected(false));
        s.input(JoyInput::Connected(true));
        assert!(s.tick(0.1, now).is_empty());
    }

    #[test]
    fn dt_is_clamped() {
        let (mut s, now) = held(1.0);
        let out = s.tick(10.0, now);
        assert_eq!(s.snapshot().values[0], Some(70));
        assert!(out.contains(&(20, 70)));
    }
}
