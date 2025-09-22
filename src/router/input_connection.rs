use crate::router::{midi_handler::MidiHandler, output_connection::OutputConnection};
use anyhow::{Result, anyhow, Error};
use log::error;
use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use wmidi::MidiMessage;
use std::sync::{Arc, Mutex};
use InputMessage::{ControllerMessage, SoftwareMessage};

pub struct InputConnection {
    connection: Option<MidiInputConnection<()>>,
}

pub enum InputMessage {
    ControllerMessage,
    SoftwareMessage,
}

impl InputConnection {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub fn connect(
        &mut self,
        name: &str,
        midi: MidiInput,
        port: &MidiInputPort,
        handler: Arc<Mutex<MidiHandler>>,
        to_controller_connection: Arc<Mutex<OutputConnection>>,
        to_software_connection: Arc<Mutex<OutputConnection>>,
        msg_type: InputMessage,
    ) -> Result<()> {
        let msg_type = msg_type;

        let connection = midi.connect(port, name, move |_timestamp, message, _data| {
            if message.is_empty() {
                return;
            }

            if let Err(err) = (|| {
                let midi_msg = MidiMessage::try_from(message)
                    .map_err(|err| anyhow!("Failed to parse MIDI message: {}", err))?;

                match msg_type {
                    ControllerMessage => {
                        let mut handler_lock = handler.lock().unwrap();
                        let mut controller_lock = to_controller_connection.lock().unwrap();
                        let mut software_lock = to_software_connection.lock().unwrap();
                        handler_lock.handle_controller_msg(midi_msg, &mut controller_lock, &mut software_lock)?;
                    },
                    SoftwareMessage => {
                        let mut handler_lock = handler.lock().unwrap();
                        let mut controller_lock = to_controller_connection.lock().unwrap();
                        handler_lock.handle_software_msg(midi_msg, &mut controller_lock)?;
                    }
                }

                Ok::<(), Error>(())
            })() {
                error!("{}", err);
            }
        }, ())?;

        let self_connection = &mut self.connection;
        *self_connection = Some(connection);

        Ok(())
    }
}
