use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JoystickConfig {
    pub channel: u8,
    #[serde(default)]
    pub device: String,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_deadzone")]
    pub deadzone: f32,
    #[serde(default = "default_curve")]
    pub curve: f32,
    pub banks: Vec<BankConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankConfig {
    pub pan: u8,
    pub tilt: u8,
    #[serde(default)]
    pub pan_fine: Option<u8>,
    #[serde(default)]
    pub tilt_fine: Option<u8>,
}

fn default_speed() -> f32 {
    2.0
}
fn default_deadzone() -> f32 {
    0.1
}
fn default_curve() -> f32 {
    2.0
}

impl JoystickConfig {
    pub fn validate(&self) -> Result<()> {
        if !(1..=16).contains(&self.channel) {
            bail!("joystick.channel must be 1..=16, got {}", self.channel);
        }
        if !(1..=12).contains(&self.banks.len()) {
            bail!(
                "joystick.banks must have 1..=12 entries, got {}",
                self.banks.len()
            );
        }
        if !(self.speed > 0.0) {
            bail!("joystick.speed must be > 0, got {}", self.speed);
        }
        if !(0.0..1.0).contains(&self.deadzone) {
            bail!(
                "joystick.deadzone must be in 0.0..1.0, got {}",
                self.deadzone
            );
        }
        if !(self.curve > 0.0) {
            bail!("joystick.curve must be > 0, got {}", self.curve);
        }
        for (i, b) in self.banks.iter().enumerate() {
            for (name, cc) in [
                ("pan", Some(b.pan)),
                ("tilt", Some(b.tilt)),
                ("pan_fine", b.pan_fine),
                ("tilt_fine", b.tilt_fine),
            ] {
                if let Some(cc) = cc
                    && cc > 127
                {
                    bail!("joystick.banks[{i}].{name} must be 0..=127, got {cc}");
                }
            }
        }
        Ok(())
    }

    /// The APC40 uses MIDI channels 1..9 for its banks.
    pub fn channel_warning(&self) -> Option<String> {
        (1..=9).contains(&self.channel).then(|| {
            format!(
                "joystick uses MIDI channel {} which the APC40 also uses (1..9); pick 10..16",
                self.channel
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "channel = 16\n[[banks]]\npan = 20\ntilt = 21\n";

    fn parse(s: &str) -> JoystickConfig {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn defaults_apply() {
        let c = parse(OK);
        assert_eq!(c.device, "");
        assert_eq!(c.speed, 2.0);
        assert_eq!(c.deadzone, 0.1);
        assert_eq!(c.curve, 2.0);
        assert_eq!(c.banks[0].pan_fine, None);
        c.validate().unwrap();
    }

    #[test]
    fn validate_rejects() {
        let bank = "[[banks]]\npan = 1\ntilt = 2\n";
        let cases = [
            (format!("channel = 0\n{bank}"), "channel"),
            (format!("channel = 17\n{bank}"), "channel"),
            (
                "channel = 16\n[[banks]]\npan = 128\ntilt = 2\n".into(),
                "pan",
            ),
            (
                "channel = 16\n[[banks]]\npan = 1\ntilt = 2\npan_fine = 200\n".into(),
                "pan_fine",
            ),
            ("channel = 16\nbanks = []\n".into(), "banks"),
            (format!("channel = 16\n{}", bank.repeat(13)), "banks"),
            (format!("deadzone = 1.0\n{OK}"), "deadzone"),
            (format!("speed = 0.0\n{OK}"), "speed"),
            (format!("curve = 0.0\n{OK}"), "curve"),
        ];
        for (toml, field) in cases {
            let err = parse(&toml).validate().unwrap_err().to_string();
            assert!(err.contains(field), "{toml:?}: {err}");
        }
    }

    #[test]
    fn channel_warning_for_apc_channels() {
        let mut c = parse(OK);
        c.channel = 5;
        assert!(c.channel_warning().unwrap().contains("channel 5"));
        c.channel = 16;
        assert!(c.channel_warning().is_none());
    }
}
