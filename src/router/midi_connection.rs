use crate::router::mapping_config::MappingConfig;
use crate::router::{
    input_connection::{
        InputConnection,
        InputMessage::{ControllerMessage, SoftwareMessage},
    },
    midi_handler::MidiHandler,
    output_connection::OutputConnection,
};
use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use midir::{MidiIO, MidiInput, MidiInputPort, MidiOutput, MidiOutputPort};
use std::sync::{Arc, Mutex};

pub struct MidiRouter {
    from_controller_connection: InputConnection,
    to_controller_connection: Arc<Mutex<OutputConnection>>,
    from_software_connection: InputConnection,
    to_software_connection: Arc<Mutex<OutputConnection>>,
    midi_handler: Arc<Mutex<MidiHandler>>,
}

struct MidiConnections {
    from_controller_name: String,
    to_controller_name: String,
    from_software_name: String,
    to_software_name: String,

    from_controller_midi: MidiInput,
    to_controller_midi: MidiOutput,
    from_software_midi: MidiInput,
    to_software_midi: MidiOutput,

    from_controller_port: MidiInputPort,
    to_controller_port: MidiOutputPort,
    from_software_port: MidiInputPort,
    to_software_port: MidiOutputPort,
}

impl MidiRouter {
    pub fn new(config: MappingConfig) -> Self {
        Self {
            from_controller_connection: InputConnection::new(),
            to_controller_connection: Arc::new(Mutex::new(OutputConnection::new())),
            from_software_connection: InputConnection::new(),
            to_software_connection: Arc::new(Mutex::new(OutputConnection::new())),
            midi_handler: Arc::new(Mutex::new(MidiHandler::new(config))),
        }
    }

    pub fn connect(&mut self, controller_name: &str, software_name: &str) -> Result<()> {
        let connections = self.setup_midi_connections(controller_name, software_name)?;

        self.connect_midi_devices(connections)?;
        Ok(())
    }

    fn setup_midi_connections(
        &self,
        controller_name: &str,
        software_name: &str,
    ) -> Result<MidiConnections> {
        let from_software_midi_name = format!("from_{}", software_name);
        let to_software_midi_name = format!("to_{}", software_name);

        let from_controller_name = format!("{}-router-input", controller_name.to_lowercase());
        let to_controller_name = format!("{}-router-output", controller_name.to_lowercase());
        let from_software_name = format!("{}-router-input", software_name.to_lowercase());
        let to_software_name = format!("{}-router-output", software_name.to_lowercase());

        // Create MIDI interfaces
        let from_controller_midi = MidiInput::new(&from_controller_name)
            .context("Failed to create controller MIDI input")?;
        let to_controller_midi = MidiOutput::new(&to_controller_name)
            .context("Failed to create controller MIDI output")?;
        let from_software_midi =
            MidiInput::new(&from_software_name).context("Failed to create software MIDI input")?;
        let to_software_midi =
            MidiOutput::new(&to_software_name).context("Failed to create software MIDI output")?;

        // Fetch ports
        let from_controller_ports = from_controller_midi.ports();
        let to_controller_ports = to_controller_midi.ports();
        let from_software_ports = from_software_midi.ports();
        let to_software_ports = to_software_midi.ports();

        // Try to get ports (gracefully)
        let from_controller_port = find_port(
            &from_controller_midi,
            &from_controller_ports,
            controller_name,
        )?;
        let to_controller_port =
            find_port(&to_controller_midi, &to_controller_ports, controller_name)?;
        let from_software_port = find_port(
            &from_software_midi,
            &from_software_ports,
            &from_software_midi_name,
        )?;
        let to_software_port = find_port(
            &to_software_midi,
            &to_software_ports,
            &to_software_midi_name,
        )?;

        /// Helper to safely find a port (with fallback + warning)
        fn find_port<P: MidiIO>(midi_io: &P, ports: &[P::Port], name: &str) -> Result<P::Port> {
            if ports.is_empty() {
                return Err(anyhow!("No MIDI ports available for '{}'", name));
            }

            if let Some(port) = ports.iter().find(|port| {
                midi_io
                    .port_name(port)
                    .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                    .unwrap_or(false)
            }) {
                Ok(port.clone())
            } else {
                warn!("No matching port for '{}', using first available.", name);
                Ok(ports[0].clone())
            }
        }

        info!(
            "Using Controller input: {}",
            from_controller_midi.port_name(&from_controller_port)?
        );
        info!(
            "Using Controller feedback output: {}",
            to_controller_midi.port_name(&to_controller_port)?
        );
        info!(
            "Using Software feedback input: {}",
            from_software_midi.port_name(&from_software_port)?
        );
        info!(
            "Using Software output: {}",
            to_software_midi.port_name(&to_software_port)?
        );

        Ok(MidiConnections {
            from_controller_name,
            to_controller_name,
            from_software_name,
            to_software_name,

            from_controller_midi,
            to_controller_midi,
            from_software_midi,
            to_software_midi,

            from_controller_port,
            to_controller_port,
            from_software_port,
            to_software_port,
        })
    }

    fn connect_midi_devices(&mut self, connections: MidiConnections) -> Result<()> {
        let handler = Arc::clone(&self.midi_handler);
        let to_controller = Arc::clone(&self.to_controller_connection);
        let to_software = Arc::clone(&self.to_software_connection);

        self.from_controller_connection.connect(
            &connections.from_controller_name,
            connections.from_controller_midi,
            &connections.from_controller_port,
            handler.clone(),
            to_controller.clone(),
            to_software.clone(),
            ControllerMessage,
        )?;

        {
            let mut to_controller_lock = to_controller.lock().unwrap();
            to_controller_lock.connect(
                &connections.to_controller_name,
                connections.to_controller_midi,
                &connections.to_controller_port,
            )?;
        }

        self.from_software_connection.connect(
            &connections.from_software_name,
            connections.from_software_midi,
            &connections.from_software_port,
            handler,
            to_controller,
            to_software,
            SoftwareMessage,
        )?;

        let mut to_software_lock = self.to_software_connection.lock().unwrap();
        to_software_lock.connect(
            &connections.to_software_name,
            connections.to_software_midi,
            &connections.to_software_port,
        )?;

        Ok(())
    }

    /*fn handle_software(
        _stamp: u64,
        message: &[u8],
        to_controller_connection: &OutputConnection,
        _to_software_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        toggle_notes: &Vec<u8>,
        _note_map: &Vec<(u8, Vec<u8>)>,
        _control_map: &Vec<(u8, Vec<u8>)>,
        _new_channel: &mut Arc<Mutex<Channel>>
    ) -> Result<()> {
        // Check if the message is empty and if its MIDI message
        if message.is_empty() {
            return Ok(());
        }

        let midi_message = MidiMessage::try_from(message)
            .map_err(|err| anyhow!("Failed to parse a MIDI message: {}", err))?;

        trace!("Software: {:?}", midi_message);

        // Process the MIDI message
        Self::match_software_message(to_controller_connection, states_map, color_map, toggle_notes, midi_message)?;

        Ok(())
    }*/

    /*fn match_controller_message(
        to_controller_connection: &OutputConnection,
        to_software_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        toggle_notes: &Vec<u8>,
        note_map: &Vec<(u8, Vec<u8>)>,
        control_map: &Vec<(u8, Vec<u8>)>,
        new_channel: &Channel,
        midi_message: MidiMessage,
        message: &[u8]
    ) -> Result<()> {
        match midi_message {
            NoteOn(channel, note, velocity) => {
                // Remap the note and check if it is a toggle note
                let new_note = Self::remap_note(note_map, &channel, note)?;

                if toggle_notes.contains(&u8::from(new_note)) {
                    // Toggle the note
                    Self::toggle_note(to_controller_connection, to_software_connection, states_map, color_map, new_channel, new_note, velocity)?;
                } else {
                    warn!("Toggle notes doesn't include note: {}", new_note);
                };
            }

            NoteOff(channel, note, _velocity) => {
                // Remap the note and check if it is a toggle note
                let new_note = Self::remap_note(note_map, &channel, note)?;

                if toggle_notes.contains(&u8::from(new_note)) {
                    // Lock the states_map and color_map to safely read and modify them (Multi-threading safety)
                    let mut states_map_lock = states_map.lock().map_err(|err| format_err!("{}", err))?;
                    let mut color_map_lock = color_map.lock().map_err(|err| format_err!("{}", err))?;

                    // Check if the states_map and color_map have the channel and refresh the LED on the controller
                    if let Some(states_map) = states_map_lock.get_mut(&new_channel.index()) {
                        if let Some(color_map) = color_map_lock.get_mut(&new_channel.index()) {
                            Self::refresh_led(to_controller_connection, states_map, color_map, new_note)?;
                        };
                    };

                    // Drop the locked maps to allow other threads to access them (Not Needed in this case, but safety firsts)
                    drop(states_map_lock);
                    drop(color_map_lock);
                } else {
                    warn!("Toggle notes doesn't include note: {}", new_note);
                };
            }

            ControlChange(channel, control, velocity) => {
                //Remap the control and send it to the software
                let new_control = Self::remap_control(control_map, &channel, control)?;
                let message = ControlChange(*new_channel, new_control, velocity);
                Self::send_midi_message(to_software_connection, message)?;
            }

            _ => {
                // For other MIDI messages, send them to the software
                let mut to_software_connection_lock = to_software_connection.connection.lock().map_err(|err| format_err!("{}", err))?;

                if let Some(to_software_connection) = to_software_connection_lock.as_mut() {
                    to_software_connection.send(message)?;
                };

                drop(to_software_connection_lock);
            }
        };

        Ok(())
    }

    fn match_software_message(
        to_controller_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        toggle_notes: &Vec<u8>,
        midi_message: MidiMessage
    ) -> Result<()> {
        // Lock the states_map and color_map to safely read and modify them (Multi-threading safety)
        let mut states_map_lock = states_map.lock().map_err(|err| format_err!("{}", err))?;
        let mut color_map_lock = color_map.lock().map_err(|err| format_err!("{}", err))?;

        match midi_message {
            NoteOn(channel, note, velocity) => {
                // Check if the note is a toggle note and check if the states_map and color_map have the channel
                if toggle_notes.contains(&u8::from(note)) {
                    if let Some(states_map) = states_map_lock.get_mut(&channel.index()) {
                        if let Some(color_map) = color_map_lock.get_mut(&channel.index()) {
                            // Set the state of the note to true in the states_map
                            states_map[u8::from(note) as usize] = true;

                            // Set the color for the note in the color_map and refresh the LED on the controller
                            color_map[u8::from(note) as usize] = u8::from(velocity);
                            Self::refresh_led(to_controller_connection, states_map, color_map, note)?;
                        };
                    };
                } else {
                    warn!("Toggle notes doesn't include note: {}", note);
                };
            }

            NoteOff(channel, note, _velocity) => {
                // Check if the note is a toggle note and check if the states_map and color_map have the channel
                if toggle_notes.contains(&u8::from(note)) {
                    if let Some(states_map) = states_map_lock.get_mut(&channel.index()) {
                        if let Some(color_map) = color_map_lock.get_mut(&channel.index()) {
                            // Set the state of the note to false in the states_map and refresh the LED on the controller
                            states_map[u8::from(note) as usize] = false;

                            Self::refresh_led(to_controller_connection, states_map, color_map, note)?;
                        };
                    };
                } else {
                    warn!("Toggle notes doesn't include note: {}", note);
                };
            }

            // No need to handle ControlChange, because my Controller doesn't have motorized faders, your problem if you have them (:
            ControlChange(_channel, _control, _velocity) => {}
            _ => {}
        };

        // Drop the locked maps to allow other threads to access them (Not Needed in this case, but safety firsts)
        drop(states_map_lock);
        drop(color_map_lock);

        Ok(())
    }

    fn toggle_note(
        to_controller_connection: &OutputConnection,
        to_software_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        channel: &Channel,
        note: Note,
        velocity: Velocity
    ) -> Result<()> {
        // Lock the states_map and color_map to safely read and modify them (Multi-threading safety)
        let mut states_map_lock = states_map.lock().map_err(|err| format_err!("{}", err))?;
        let mut color_map_lock = color_map.lock().map_err(|err| format_err!("{}", err))?;

        // Check if the states_map and color_map have the channel
        if let Some(states_map) = states_map_lock.get_mut(&channel.index()) {
            if let Some(color_map) = color_map_lock.get_mut(&channel.index()) {
                // Get the state corresponding to the note and toggle the state of the note in the states_map
                if let Some(state) = states_map.get_mut(u8::from(note) as usize) {
                    *state = !*state;

                    // Send the processed MIDI message to the software and refresh the LED of the note on the controller
                    let message = NoteOn(*channel, note, velocity);

                    Self::send_midi_message(to_software_connection, message)?;
                    Self::refresh_led(to_controller_connection, states_map, color_map, note)?;
                };
            };
        };

        // Drop the locked maps to allow other threads to access them (Not Needed in this case, but safety firsts)
        drop(states_map_lock);
        drop(color_map_lock);

        Ok(())
    }

    fn refresh_leds(
        to_controller_connection: &OutputConnection,
        states_map: &mut Vec<bool>,
        color_map: &mut Vec<u8>,
        toggle_notes: &Vec<u8>
    ) -> Result<()> {
        // Iterate through the notes and refresh the LED for each note
        for note_u8 in toggle_notes.iter() {
            let note = Note::from_u8_lossy(*note_u8);
            Self::refresh_led(to_controller_connection, states_map, color_map, note)?;
        };

        Ok(())
    }

    fn refresh_led(
        to_controller_connection: &OutputConnection,
        states_map: &mut Vec<bool>,
        color_map: &mut Vec<u8>,
        note: Note
    ) -> Result<()> {
        // Check if the states_map and color_map have the note
        if let Some(state) = states_map.get(u8::from(note) as usize) {
            if let Some(color) = color_map.get(u8::from(note) as usize) {
                // If the state is true, send a NoteOn message with LED_ON velocity (color), otherwise send a NoteOn with the velocity (color) from the color_map to the controller
                let message = NoteOn(
                    Channel::Ch1,
                    note,
                    if *state { LED_ON } else { Velocity::from_u8_lossy(*color) }
                );

                Self::send_midi_message(to_controller_connection, message)?;
            };
        };

        // Sleep for a short duration to avoid overwhelming the MIDI connection
        thread::sleep(Duration::from_micros(25));

        Ok(())
    }

    fn send_midi_message(
        output_connection: &OutputConnection,
        message: MidiMessage
    ) -> Result<()> {
        // Lock the output_connection to safely send messages (Multi-threading safety)
        let mut output_connection_lock = output_connection.connection.lock().map_err(|err| format_err!("{}", err))?;

        // Send the message
        if let Some(output_connection) = output_connection_lock.as_mut() {
            let mut buffer = [0_u8; 3];
            let length = message.copy_to_slice(&mut buffer)?;

            output_connection.send(&buffer[..length])?;
        };

        // Drop the locked connection to allow other threads to access them (Not Needed in this case, but safety firsts)
        drop(output_connection_lock);

        Ok(())
    }

    fn remap_note(
        note_map: &Vec<(u8, Vec<u8>)>,
        channel: &Channel,
        note: Note
    ) -> Result<Note> {
        // Iterate through the note_map to find the remapped_notes for the original_note / note
        for (original_note, remapped_notes) in note_map {
            if *original_note == u8::from(note) {
                // Check if the channel is within the bounds of the remapped_notes and return the remapped note
                if channel.index() < remapped_notes.len() as u8 {
                    return Ok(Note::from_u8_lossy(remapped_notes[channel.index() as usize]));
                };
            };
        };

        // If no remapping is found, return the original note
        Ok(note)
    }

    fn remap_control(
        control_map: &Vec<(u8, Vec<u8>)>,
        channel: &Channel,
        control: ControlFunction
    ) -> Result<ControlFunction> {
        // Iterate through the control_map to find the remapped_controls for the original_control / control
        for (original_control, remapped_controls) in control_map {
            if *original_control == u8::from(control) {
                // Check if the channel is within the bounds of the remapped_controls and return the remapped control
                if channel.index() < remapped_controls.len() as u8 {
                    return Ok(ControlFunction::from(U7::from_u8_lossy(remapped_controls[channel.index() as usize])));
                };
            };
        };

        // If no remapping is found, return the original control
        Ok(control)
    }*/
}
