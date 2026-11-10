use anyhow::{Result, anyhow};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};
use wmidi::MidiMessage;

pub struct OutputConnection {
    pub connection: Option<MidiOutputConnection>,
    /// Test-only record of every message passed to `send`.
    #[cfg(test)]
    pub sent: Vec<Vec<u8>>,
}

impl OutputConnection {
    pub fn new() -> Self {
        Self {
            connection: None,
            #[cfg(test)]
            sent: Vec::new(),
        }
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
        let mut buffer = [0_u8; 3];
        let length = msg.copy_to_slice(&mut buffer)?;

        #[cfg(test)]
        self.sent.push(buffer[..length].to_vec());

        if let Some(connection) = &mut self.connection {
            connection.send(&buffer[..length])?;
        }

        Ok(())
    }
}
