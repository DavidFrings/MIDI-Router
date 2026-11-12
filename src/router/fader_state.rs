use std::time::{Duration, Instant};
use wmidi::Channel;

pub const FADER_COUNT: usize = 9;
pub const PICKUP_WINDOW: u8 = 2;
pub const ECHO_GUARD: Duration = Duration::from_millis(200);

/// Fader values per bank (0..9) with soft takeover and an echo guard.
#[derive(Default)]
pub struct FaderState {
    values: [[Option<u8>; FADER_COUNT]; FADER_COUNT],
    /// One physical fader per index, shared by all banks.
    physical: [Option<u8>; FADER_COUNT],
    last_sent: [[Option<Instant>; FADER_COUNT]; FADER_COUNT],
}

impl FaderState {
    /// true = accept and send.
    pub fn controller_move(&mut self, bank: usize, index: usize, value: u8, now: Instant) -> bool {
        if bank >= FADER_COUNT || index >= FADER_COUNT {
            return false; // banks 10-16 have no fader state
        }
        let prev = self.physical[index];
        self.physical[index] = Some(value);
        let accept = match self.values[bank][index] {
            None => true,
            Some(stored) => {
                prev == Some(stored) // already in sync
                    || value.abs_diff(stored) <= PICKUP_WINDOW
                    || prev.is_some_and(|p| (p < stored) != (value < stored)) // crossed
            }
        };
        if accept {
            self.values[bank][index] = Some(value);
            self.last_sent[bank][index] = Some(now);
        }
        accept
    }

    pub fn remote_set(&mut self, bank: usize, index: usize, value: u8, now: Instant) {
        if bank >= FADER_COUNT || index >= FADER_COUNT {
            return;
        }
        self.values[bank][index] = Some(value);
        self.last_sent[bank][index] = Some(now);
    }

    /// false = ignored by the echo guard.
    // shortcut: unused until Task 10 wires Daslight feedback.
    #[allow(dead_code)]
    pub fn software_set(&mut self, bank: usize, index: usize, value: u8, now: Instant) -> bool {
        if bank >= FADER_COUNT || index >= FADER_COUNT {
            return false;
        }
        if self.last_sent[bank][index].is_some_and(|t| now.saturating_duration_since(t) < ECHO_GUARD) {
            return false;
        }
        self.values[bank][index] = Some(value);
        true
    }

    pub fn value(&self, bank: usize, index: usize) -> Option<u8> {
        *self.values.get(bank)?.get(index)?
    }
}

/// CC 7 on Ch1-Ch8 -> 0..7, CC 14 on any channel -> 8 (master).
pub fn controller_fader_index(channel: Channel, cc: u8) -> Option<usize> {
    match (cc, channel.index() as usize) {
        (7, i) if i < 8 => Some(i),
        (14, _) => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_controller_move_is_accepted() {
        let mut s = FaderState::default();
        assert!(s.controller_move(0, 0, 40, Instant::now()));
        assert_eq!(s.value(0, 0), Some(40));
    }

    #[test]
    fn continuous_controller_moves_stay_accepted() {
        let mut s = FaderState::default();
        let t = Instant::now();
        for v in [60, 61, 62] {
            assert!(s.controller_move(0, 0, v, t));
        }
    }

    #[test]
    fn controller_ignored_after_remote_change_until_pickup() {
        let mut s = FaderState::default();
        let t = Instant::now();
        s.controller_move(0, 0, 20, t);
        s.remote_set(0, 0, 100, t);
        assert!(!s.controller_move(0, 0, 50, t));
        assert_eq!(s.value(0, 0), Some(100));
        assert!(s.controller_move(0, 0, 98, t));
    }

    #[test]
    fn controller_picks_up_when_crossing() {
        let mut s = FaderState::default();
        let t = Instant::now();
        s.controller_move(0, 0, 90, t);
        s.remote_set(0, 0, 100, t);
        assert!(s.controller_move(0, 0, 110, t));
    }

    #[test]
    fn page_change_requires_pickup() {
        let mut s = FaderState::default();
        let t = Instant::now();
        s.controller_move(0, 0, 20, t);
        s.remote_set(1, 0, 80, t);
        assert!(!s.controller_move(1, 0, 25, t));
    }

    #[test]
    fn software_echo_within_200ms_is_ignored() {
        let mut s = FaderState::default();
        let t = Instant::now();
        s.remote_set(0, 0, 50, t);
        assert!(!s.software_set(0, 0, 70, t + Duration::from_millis(100)));
        assert_eq!(s.value(0, 0), Some(50));
    }

    #[test]
    fn software_after_200ms_is_applied() {
        let mut s = FaderState::default();
        let t = Instant::now();
        s.remote_set(0, 0, 50, t);
        assert!(s.software_set(0, 0, 70, t + Duration::from_millis(250)));
        assert_eq!(s.value(0, 0), Some(70));
    }

    #[test]
    fn out_of_range_bank_does_not_panic() {
        let mut s = FaderState::default();
        let t = Instant::now();
        assert!(!s.controller_move(9, 0, 1, t));
        assert!(!s.controller_move(0, 9, 1, t));
        s.remote_set(15, 0, 1, t);
        assert!(!s.software_set(9, 0, 1, t));
        assert_eq!(s.value(9, 0), None);
        assert_eq!(s.value(0, 9), None);
    }

    #[test]
    fn controller_fader_index_maps_apc_faders() {
        assert_eq!(controller_fader_index(Channel::Ch3, 7), Some(2));
        assert_eq!(controller_fader_index(Channel::Ch1, 14), Some(8));
        assert_eq!(controller_fader_index(Channel::Ch1, 16), None);
        assert_eq!(controller_fader_index(Channel::Ch9, 7), None);
    }
}
