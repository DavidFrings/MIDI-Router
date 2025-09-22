use anyhow::Result;
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};

pub struct OutputConnection {
    pub connection: Option<MidiOutputConnection>,
}

impl OutputConnection {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub fn connect(&mut self, name: &str, midi: MidiOutput, port: &MidiOutputPort) -> Result<()> {
        let connection = midi.connect(port, name)?;

        let self_connection = &mut self.connection;
        *self_connection = Some(connection);

        Ok(())
    }
}
