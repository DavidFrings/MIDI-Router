//! Input report parser for the Thrustmaster TCA Sidestick (layout from the probe log).

use super::state::{Axis, JoyInput};

pub const VID: u16 = 0x044F;

/// Remembers the last report so `parse` only emits changes.
#[derive(Default)]
pub struct TcaParser {
    x: Option<u16>,
    y: Option<u16>,
    hat: (i8, i8),
    buttons: u32,
}

impl TcaParser {
    pub fn parse(&mut self, report: &[u8]) -> Vec<JoyInput> {
        let mut out = Vec::new();
        if report.len() < 11 || report[0] != 0x01 {
            return out;
        }
        // Bytes 5 (twist) and 6 (slider) are ignored on purpose.
        let x = u16::from_le_bytes([report[1], report[2]]);
        let y = u16::from_le_bytes([report[3], report[4]]);
        if self.x != Some(x) {
            self.x = Some(x);
            let value = ((x as f32 - 8192.0) / 8191.0).clamp(-1.0, 1.0);
            out.push(JoyInput::Axis {
                axis: Axis::X,
                value,
            });
        }
        if self.y != Some(y) {
            self.y = Some(y);
            let value = ((8192.0 - y as f32) / 8192.0).clamp(-1.0, 1.0);
            out.push(JoyInput::Axis {
                axis: Axis::Y,
                value,
            });
        }
        let hat = match report[7] & 0x0F {
            0 => (0, 1),
            1 => (1, 1),
            2 => (1, 0),
            3 => (1, -1),
            4 => (0, -1),
            5 => (-1, -1),
            6 => (-1, 0),
            7 => (-1, 1),
            _ => (0, 0),
        };
        if hat != self.hat {
            self.hat = hat;
            out.push(JoyInput::Hat { x: hat.0, y: hat.1 });
        }
        let buttons = u32::from_le_bytes([report[8], report[9], report[10], 0]) & 0x1_FFFF;
        let changed = buttons ^ self.buttons;
        self.buttons = buttons;
        for k in 0..17 {
            if changed & (1 << k) != 0 {
                out.push(JoyInput::Button {
                    index: k + 1,
                    pressed: buttons & (1 << k) != 0,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(hex: &str) -> Vec<u8> {
        let mut r: Vec<u8> = hex
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        r.resize(64, 0);
        r
    }

    const REST: &str = "01 04 20 2D 20 6C FF 0F 00 00 01";

    fn axis(inputs: &[JoyInput], which: Axis) -> Option<f32> {
        inputs.iter().find_map(|i| match i {
            JoyInput::Axis { axis, value } if *axis == which => Some(*value),
            _ => None,
        })
    }

    fn button(index: u8, pressed: bool) -> JoyInput {
        JoyInput::Button { index, pressed }
    }

    #[test]
    fn rest_report_emits_axes_and_throttle_button_once() {
        let mut p = TcaParser::default();
        let out = p.parse(&report(REST));
        assert!((axis(&out, Axis::X).unwrap() - 0.0005).abs() < 0.0001);
        assert!((axis(&out, Axis::Y).unwrap() + 0.0055).abs() < 0.0001);
        assert!(out.contains(&button(17, true)));
        assert!(!out.iter().any(|i| matches!(i, JoyInput::Hat { .. })));
        assert_eq!(out.len(), 3);
        assert!(p.parse(&report(REST)).is_empty());
    }

    #[test]
    fn x_extremes() {
        let mut p = TcaParser::default();
        let out = p.parse(&report("01 AD 00 2B 22 51 FF 0F 00 00 01"));
        assert!((axis(&out, Axis::X).unwrap() + 0.9789).abs() < 0.001);
        let out = p.parse(&report("01 FF 3F AE 1E 00 FF 0F 00 00 01"));
        assert_eq!(axis(&out, Axis::X), Some(1.0));
    }

    #[test]
    fn trigger_press_and_release() {
        let mut p = TcaParser::default();
        p.parse(&report(REST));
        let out = p.parse(&report("01 07 20 69 20 00 FF 0F 01 00 01"));
        assert!(out.contains(&button(1, true)));
        let out = p.parse(&report(REST));
        assert!(out.contains(&button(1, false)));
    }

    #[test]
    fn base_buttons_15_and_16() {
        let mut p = TcaParser::default();
        p.parse(&report(REST));
        let out = p.parse(&report("01 17 20 DE 20 43 FF 0F 00 C0 01"));
        assert!(out.contains(&button(15, true)));
        assert!(out.contains(&button(16, true)));
    }

    #[test]
    fn button_2_is_bit_1_of_byte_8() {
        let mut p = TcaParser::default();
        let out = p.parse(&report("01 00 20 00 20 80 80 0F 02 00 00"));
        assert!(out.contains(&button(2, true)));
        assert!(
            !out.iter()
                .any(|i| matches!(i, JoyInput::Button { index: 1, .. }))
        );
    }

    #[test]
    fn hat_directions() {
        let mut p = TcaParser::default();
        p.parse(&report(REST));
        let out = p.parse(&report("01 3A 20 10 21 05 FF 02 00 00 01"));
        assert!(out.contains(&JoyInput::Hat { x: 1, y: 0 }));
        let out = p.parse(&report("01 3A 20 10 21 05 FF 04 01 00 01"));
        assert!(out.contains(&JoyInput::Hat { x: 0, y: -1 }));
        assert!(out.contains(&button(1, true)));
        let out = p.parse(&report("01 3A 20 10 21 05 FF 0F 01 00 01"));
        assert_eq!(out, vec![JoyInput::Hat { x: 0, y: 0 }]);
    }

    #[test]
    fn foreign_or_short_reports_are_ignored() {
        let mut p = TcaParser::default();
        let mut other = report(REST);
        other[0] = 0x02;
        assert!(p.parse(&other).is_empty());
        assert!(p.parse(&report(REST)[..5]).is_empty());
        // state unchanged: the first real report still emits everything
        assert_eq!(p.parse(&report(REST)).len(), 3);
    }
}
