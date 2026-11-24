use super::state::JoyInput;

pub trait JoystickSource {
    fn poll(&mut self) -> Vec<JoyInput>;
}

struct NoDevice;

impl JoystickSource for NoDevice {
    fn poll(&mut self) -> Vec<JoyInput> {
        Vec::new()
    }
}

// shortcut: yields nothing until the real HID backend lands (Task 7, needs hardware data).
pub fn open(_device: &str) -> Box<dyn JoystickSource + Send> {
    Box::new(NoDevice)
}
