use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use log::error;
use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use wmidi::Channel;
use crate::router::output_connection::OutputConnection;

pub struct InputConnection {
    connection: Option<MidiInputConnection<()>>
}

impl InputConnection {
    pub fn new() -> Self {
        Self {
            connection: None
        }
    }

    pub fn handle(
        &mut self,
        handler: fn(
            u64, &[u8], 
            &OutputConnection, 
            &OutputConnection, 
            &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>, 
            &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>, 
            &Vec<u8>, 
            &Vec<(u8, Vec<u8>)>, 
            &Vec<(u8, Vec<u8>)>, 
            &mut Arc<Mutex<Channel>>
        ) -> Result<()>,
        midi_io: MidiInput,
        port: &MidiInputPort,
        name: String,
        to_controller_connection: &OutputConnection,
        to_software_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        toggle_notes: &Vec<u8>,
        note_map: &Vec<(u8, Vec<u8>)>,
        control_map: &Vec<(u8, Vec<u8>)>,
        new_channel: &mut Arc<Mutex<Channel>>
    ) -> Result<()> {

        let input_connection = midi_io.connect(port, name.as_str(), move |stamp, message, _| {
            if let Err(err) = handler(stamp, message, to_controller_connection, to_software_connection, states_map, color_map, toggle_notes, note_map, control_map, new_channel) {
                error!("Error handling MIDI message: {}", err);
            }
        },())?;

        self.connection = Some(input_connection);
        Ok(())
    }
}