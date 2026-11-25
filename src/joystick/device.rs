use super::state::JoyInput;

pub trait JoystickSource {
    fn poll(&mut self) -> Vec<JoyInput>;
}

#[cfg(not(windows))]
struct NoDevice;

#[cfg(not(windows))]
impl JoystickSource for NoDevice {
    fn poll(&mut self) -> Vec<JoyInput> {
        Vec::new()
    }
}

// shortcut: no joystick backend outside Windows (raw HID via hidapi is Windows-only), add one if Linux is needed.
#[cfg(not(windows))]
pub fn open(_device: &str) -> Box<dyn JoystickSource + Send> {
    Box::new(NoDevice)
}

#[cfg(windows)]
pub fn open(device: &str) -> Box<dyn JoystickSource + Send> {
    Box::new(hid::HidSource::new(device))
}

/// Raw HID reads work without window focus (unlike the Windows game-controller APIs).
#[cfg(windows)]
mod hid {
    use std::time::{Duration, Instant};

    use hidapi::{HidApi, HidDevice};
    use log::{info, warn};

    use super::JoystickSource;
    use crate::joystick::{
        state::JoyInput,
        tca::{self, TcaParser},
    };

    const RETRY: Duration = Duration::from_secs(1);

    pub struct HidSource {
        api: Option<HidApi>,
        dev: Option<HidDevice>,
        parser: TcaParser,
        filter: String,
        next_try: Instant,
        /// A connect error was logged; stay quiet on the 1 s retries until the next connect.
        quiet: bool,
    }

    impl HidSource {
        pub fn new(filter: &str) -> Self {
            Self {
                api: None,
                dev: None,
                parser: TcaParser::default(),
                filter: filter.to_string(),
                next_try: Instant::now(),
                quiet: false,
            }
        }

        /// Opens the first matching stick; `Ok(None)` when none is plugged in.
        fn find(&mut self) -> Result<Option<(HidDevice, String)>, String> {
            if self.api.is_none() {
                self.api = Some(HidApi::new().map_err(|e| format!("HID init failed: {e}"))?);
            }
            let Some(api) = self.api.as_mut() else {
                return Ok(None);
            };
            api.refresh_devices()
                .map_err(|e| format!("HID device scan failed: {e}"))?;
            let filter = self.filter.as_str();
            let Some(info) = api.device_list().find(|d| {
                d.vendor_id() == tca::VID
                    && d.usage_page() == 1
                    && matches!(d.usage(), 4 | 5)
                    && d.product_string().unwrap_or("").contains(filter)
            }) else {
                return Ok(None);
            };
            let label = format!(
                "{} ({:?})",
                info.product_string().unwrap_or("?"),
                info.path()
            );
            let dev = api
                .open_path(info.path())
                .map_err(|e| format!("cannot open {label}: {e}"))?;
            dev.set_blocking_mode(false)
                .map_err(|e| format!("cannot set non-blocking mode on {label}: {e}"))?;
            Ok(Some((dev, label)))
        }
    }

    impl JoystickSource for HidSource {
        fn poll(&mut self) -> Vec<JoyInput> {
            let mut out = Vec::new();
            if self.dev.is_none() {
                let now = Instant::now();
                if now < self.next_try {
                    return out;
                }
                self.next_try = now + RETRY;
                match self.find() {
                    Ok(Some((dev, label))) => {
                        info!("joystick connected: {label}");
                        self.dev = Some(dev);
                        self.parser = TcaParser::default();
                        self.quiet = false;
                        out.push(JoyInput::Connected(true));
                    }
                    Ok(None) => return out,
                    Err(e) => {
                        if !self.quiet {
                            warn!("joystick: {e}");
                            self.quiet = true;
                        }
                        return out;
                    }
                }
            }
            let mut buf = [0u8; 64];
            while let Some(dev) = &self.dev {
                match dev.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => out.extend(self.parser.parse(&buf[..n])),
                    Err(e) => {
                        warn!("joystick disconnected: {e}");
                        self.dev = None;
                        out.push(JoyInput::Connected(false));
                    }
                }
            }
            out
        }
    }
}
