// shortcut: used by the API in Task 5
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MAX_FAILS: u32 = 5;
pub const LOCK_TIME: Duration = Duration::from_secs(60);

pub fn valid_pin(pin: &str) -> bool {
    pin.len() >= 4 && pin.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Default)]
pub struct Lockout(Mutex<HashMap<IpAddr, (u32, Option<Instant>)>>);

impl Lockout {
    /// true = attempt allowed.
    pub fn check(&self, ip: IpAddr, now: Instant) -> bool {
        let mut m = self.0.lock().unwrap();
        match m.get(&ip) {
            Some((_, Some(until))) if now < *until => false,
            Some((_, Some(_))) => {
                m.remove(&ip); // lock expired: reset counter
                true
            }
            _ => true,
        }
    }

    pub fn fail(&self, ip: IpAddr, now: Instant) {
        let mut m = self.0.lock().unwrap();
        let e = m.entry(ip).or_insert((0, None));
        e.0 += 1;
        if e.0 >= MAX_FAILS {
            e.1 = Some(now + LOCK_TIME);
        }
    }

    pub fn success(&self, ip: IpAddr) {
        self.0.lock().unwrap().remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP: &str = "192.168.1.10";

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn valid_pin_rules() {
        assert!(valid_pin("1234"));
        assert!(valid_pin("0123"));
        assert!(!valid_pin("123"));
        assert!(!valid_pin("12a4"));
        assert!(!valid_pin(""));
    }

    #[test]
    fn five_fails_lock_for_60s() {
        let l = Lockout::default();
        let t = Instant::now();
        for _ in 0..MAX_FAILS {
            l.fail(ip(IP), t);
        }
        assert!(!l.check(ip(IP), t + Duration::from_secs(1)));
        assert!(l.check(ip(IP), t + Duration::from_secs(61)));
    }

    #[test]
    fn success_resets_counter() {
        let l = Lockout::default();
        let t = Instant::now();
        for _ in 0..4 {
            l.fail(ip(IP), t);
        }
        l.success(ip(IP));
        for _ in 0..4 {
            l.fail(ip(IP), t);
        }
        assert!(l.check(ip(IP), t));
    }

    #[test]
    fn lock_is_per_ip() {
        let l = Lockout::default();
        let t = Instant::now();
        for _ in 0..MAX_FAILS {
            l.fail(ip(IP), t);
        }
        assert!(l.check(ip("192.168.1.11"), t));
    }
}
