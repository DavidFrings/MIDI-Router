use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use anyhow::{anyhow, format_err, Context, Result};
use log::{debug, info, trace, warn};
use midir::{MidiInput, MidiOutput, MidiIO, MidiOutputConnection};
use wmidi::{Channel, ControlFunction, MidiMessage, MidiMessage::{ControlChange, NoteOn, NoteOff}, Note, Velocity, U7};
use crate::LED_ON;
use crate::router::{input_connection::InputConnection, output_connection::OutputConnection};

pub struct MidiRouter {
    from_controller_connection: InputConnection,
    to_controller_connection: OutputConnection,
    from_software_connection: InputConnection,
    to_software_connection: OutputConnection,
    states_map: Arc<Mutex<HashMap<u8, Vec<bool>>>>,
    color_map: Arc<Mutex<HashMap<u8, Vec<u8>>>>,
    toggle_notes: Vec<u8>,
    note_map: Vec<(u8, Vec<u8>)>,
    control_map: Vec<(u8, Vec<u8>)>,
    default_channel: Arc<Mutex<Channel>>
}

impl MidiRouter {
    pub fn connect(&mut self, controller_name: &str, software_name: &str) -> Result<()> {
        let from_software_midi_name = format!("from_{}", software_name);
        let to_software_midi_name = format!("to_{}", software_name);

        let from_controller_name = format!("{}-router-input", controller_name.to_lowercase());
        let to_controller_name = format!("{}-router-output", controller_name.to_lowercase());
        let from_software_name = format!("{}-router-input", software_name.to_lowercase());
        let to_software_name = format!("{}-router-output", software_name.to_lowercase());

        let from_controller = MidiInput::new(from_controller_name.as_str())?;
        let to_controller = MidiOutput::new(to_controller_name.as_str())?;
        let from_software = MidiInput::new(from_software_name.as_str())?;
        let to_software = MidiOutput::new(to_software_name.as_str())?;

        let from_controller_ports = from_controller.ports();
        let to_controller_ports = to_controller.ports();
        let from_software_ports = from_software.ports();
        let to_software_ports = to_software.ports();

        let from_controller_port = &Self::find_port(&from_controller, &from_controller_ports, controller_name)?;
        let to_controller_port = &Self::find_port(&to_controller, &to_controller_ports, controller_name)?;
        let from_software_port = &Self::find_port(&from_software, &from_software_ports, from_software_midi_name.as_str())?;
        let to_software_port = &Self::find_port(&to_software, &to_software_ports, to_software_midi_name.as_str())?;

        info!("Using MIDI input: {}", from_controller.port_name(from_controller_port)?);
        info!("Using MIDI feedback output: {}", to_controller.port_name(to_controller_port)?);
        info!("Using Daslight feedback input: {}", from_software.port_name(from_software_port)?);
        info!("Using Daslight output: {}", to_software.port_name(to_software_port)?);

        let from_controller_connection = &mut self.from_controller_connection;
        let to_controller_connection = &self.to_controller_connection;
        let from_software_connection = &mut self.from_software_connection;
        let to_software_connection = &self.to_software_connection;

        let mut states_map = Arc::clone(&self.states_map);
        let mut color_map = Arc::clone(&self.color_map);
        let toggle_notes = &self.toggle_notes;
        let note_map = &self.note_map;
        let control_map = &self.control_map;
        let mut new_channel = Arc::clone(&self.default_channel);

        from_controller_connection.handle(
            Self::handle_controller,
            from_controller,
            from_controller_port,
            from_controller_name,
            to_controller_connection,
            to_software_connection,
            &mut states_map,
            &mut color_map,
            &toggle_notes,
            &note_map,
            &control_map,
            &mut new_channel
        )?;
        to_controller_connection.lock(to_controller, to_controller_port, to_controller_name)?;
        from_software_connection.handle(
            Self::handle_software,
            from_software,
            from_software_port,
            from_software_name,
            to_controller_connection,
            to_software_connection,
            &mut states_map,
            &mut color_map,
            &toggle_notes,
            &note_map,
            &control_map,
            &mut new_channel
        )?;
        to_software_connection.lock(to_software, to_software_port, to_software_name)?;
        
        Ok(())
    }

    pub fn new() -> Self {
        let from_controller_connection = InputConnection::new();
        let to_controller_connection = OutputConnection::new();
        let from_software_connection = InputConnection::new();
        let to_software_connection = OutputConnection::new();

        Self {
            from_controller_connection,
            to_controller_connection,
            from_software_connection,
            to_software_connection,
            states_map: Arc::new(Mutex::new((0..=8).map(|i| (i, vec![false; 128])).collect::<HashMap<_, _>>())),
            color_map: Arc::new(Mutex::new((0..=8).map(|i| (i, vec![0; 128])).collect::<HashMap<_, _>>())),
            toggle_notes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 81, 82, 83, 84, 85, 86, 105, 106, 107, 108, 109, 110, 111, 112],
            note_map: vec![
                (48, vec![40, 41, 42, 43, 44, 45, 46, 47]),
                (49, vec![68, 69, 70, 71, 72, 73, 74, 75]),
                (50, vec![53, 54, 55, 56, 76, 77, 78, 79]),
                (52, vec![105, 106, 107, 108, 109, 110, 111, 112]),
                (66, vec![113, 114, 115, 116, 117, 118, 119, 120])
            ],
            control_map: vec![
                (7, vec![0, 1, 2, 3, 4, 5, 9, 10])
            ],
            default_channel: Arc::new(Mutex::new(Channel::Ch1)),
        }
    }

    fn find_port<P: MidiIO> (midi_io: &P, ports: &[P::Port], name: &str) -> Result<P::Port> {
        ports
            .iter()
            .find(|port| {
                midi_io
                    .port_name(port)
                    .map(|n| n.contains(name))
                    .unwrap_or(false)
            })
            .cloned()
            .context(format!("Could not find a MIDI device containing '{}'", name))
    }

    fn handle_controller(
        _stamp: u64,
        message: &[u8],
        to_controller_connection: &OutputConnection,
        to_software_connection: &OutputConnection,
        states_map: &mut Arc<Mutex<HashMap<u8, Vec<bool>>>>,
        color_map: &mut Arc<Mutex<HashMap<u8, Vec<u8>>>>,
        toggle_notes: &Vec<u8>,
        note_map: &Vec<(u8, Vec<u8>)>,
        control_map: &Vec<(u8, Vec<u8>)>,
        new_channel: &mut Arc<Mutex<Channel>>
    ) -> Result<()> {
        // Check if the message is empty and if its MIDI message
        if message.is_empty() {
            return Ok(());
        }

        let midi_message = MidiMessage::try_from(message)
            .map_err(|err| anyhow!("Failed to parse a MIDI message: {}", err))?;

        trace!("Controller: {:?}", midi_message);

        // Lock the new_channel to safely read and modify (Multi-threading safety)
        let mut new_channel_lock = new_channel.lock().map_err(|err| format_err!("{}", err))?;

        // Check if it is a Site Change
        if let ControlChange(channel, control, _) = midi_message {
            if u8::from(control) == 16 { // 16 is the first knob that sends a signal on changing the site
                if channel != *new_channel_lock {
                    *new_channel_lock = channel;

                    // Lock the states_map and color_map to safely read and modify them (Multi-threading safety)
                    let mut states_map_lock = states_map.lock().map_err(|err| format_err!("{}", err))?;
                    let mut color_map_lock = color_map.lock().map_err(|err| format_err!("{}", err))?;

                    if let Some(states_map) = states_map_lock.get_mut(&new_channel_lock.index()) {
                        if let Some(color_map) = color_map_lock.get_mut(&new_channel_lock.index()) {
                            Self::refresh_leds(to_controller_connection, states_map, color_map, toggle_notes)?;
                        };
                    };

                    // Drop the locked maps to allow other threads to access them (Not Needed in this case, but safety firsts)
                    drop(states_map_lock);
                    drop(color_map_lock);

                    debug!("New Site: {}", format!("{:?}", channel).replace("Ch", ""));
                }
            }
        }

        // Process the MIDI message
        Self::match_controller_message(to_controller_connection, to_software_connection, states_map, color_map, toggle_notes, note_map, control_map, &*new_channel_lock, midi_message, message)?;

        // Drop the locked channel to allow other threads to access them (Not Needed in this case, but safety firsts)
        drop(new_channel_lock);
        
        Ok(())
    }

    fn handle_software(
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
    }

    fn match_controller_message(
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
    }
}



