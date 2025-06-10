use std::sync::{Arc, Mutex};
use anyhow::{format_err, Result};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};

pub struct OutputConnection {
    pub connection: Arc<Mutex<Option<MidiOutputConnection>>>
}

impl OutputConnection {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None))
        }
    }

    pub fn lock(&self, midi_io: MidiOutput, port: &MidiOutputPort, name: String) -> Result<()> {
        let connection = midi_io.connect(port, name.as_str())?;

        {
            let mut midi = self.connection.lock().map_err(|err| format_err!("{}", err))?;
            *midi = Some(connection);
        }

        Ok(())
    }
}