use anyhow::{Result, anyhow};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};
use wmidi::MidiMessage;

pub struct OutputConnection {
    pub connection: Option<MidiOutputConnection>,
}

impl OutputConnection {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub fn connect(&mut self, name: &str, midi: MidiOutput, port: &MidiOutputPort) -> Result<()> {
        let connection = midi
            .connect(port, name)
            .map_err(|err| anyhow!("{}", err))?;

        let self_connection = &mut self.connection;
        *self_connection = Some(connection);

        Ok(())
    }

    /// Sends `msg` if connected; no-op otherwise.
    pub fn send(&mut self, msg: MidiMessage) -> Result<()> {
        if let Some(connection) = &mut self.connection {
            let mut buffer = [0_u8; 3];
            let length = msg.copy_to_slice(&mut buffer)?;
            connection.send(&buffer[..length])?;
        }

        Ok(())
    }
}
