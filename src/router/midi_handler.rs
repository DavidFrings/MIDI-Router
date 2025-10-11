use crate::router::{
    led_controller::LedController, mapping_config::MappingConfig,
    output_connection::OutputConnection, state_manager::StateManager,
};
use anyhow::Result;
use log::{debug, trace, warn};
use wmidi::{
    Channel, MidiMessage,
    MidiMessage::{ControlChange, NoteOff, NoteOn},
    Note, Velocity,
};

pub struct MidiHandler {
    state_manager: StateManager,
    mapping_config: MappingConfig,
    led_controller: LedController,
}

impl MidiHandler {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state_manager: StateManager::new(),
            mapping_config: MappingConfig::new()?,
            led_controller: LedController::new(),
        })
    }

    pub fn get_state_manager(&self) -> &StateManager {
        &self.state_manager
    }

    pub fn handle_controller_msg(
        &mut self,
        msg: MidiMessage,
        to_controller_connection: &mut OutputConnection,
        to_software_connection: &mut OutputConnection,
    ) -> Result<()> {
        trace!("Received MIDI message from Controller: {:?}", msg);

        self.handle_site_change(&msg, to_controller_connection)?; // Check if user wants to change site
        self.process_controller_message(msg, to_controller_connection, to_software_connection)?;

        Ok(())
    }

    pub fn handle_software_msg(
        &mut self,
        msg: MidiMessage,
        to_controller_connection: &mut OutputConnection,
    ) -> Result<()> {
        trace!("Received MIDI message from Software: {:?}", msg);

        self.process_software_message(msg, to_controller_connection)?;

        Ok(())
    }

    fn handle_site_change(
        &mut self,
        msg: &MidiMessage,
        to_controller_connection: &mut OutputConnection,
    ) -> Result<()> {
        if let ControlChange(channel, control, _velocity) = msg {
            if u8::from(*control) == 16 {
                let current_bank = self.state_manager.get_current_bank();

                if channel != current_bank {
                    self.state_manager.set_current_bank(*channel);

                    self.led_controller.refresh_all_leds(
                        to_controller_connection,
                        &mut self.state_manager,
                        channel,
                        self.mapping_config.get_toggle_notes(),
                    )?;

                    debug!("New Site: {}", format!("{:?}", channel).replace("Ch", ""));
                }
            }
        }

        Ok(())
    }

    fn process_controller_message(
        &mut self,
        midi_message: MidiMessage,
        to_controller_connection: &mut OutputConnection,
        to_software_connection: &mut OutputConnection,
    ) -> Result<()> {
        let current_bank = &self.state_manager.get_current_bank().clone();
        match midi_message {
            NoteOn(channel, note, velocity) => {
                let remapped_note = self.mapping_config.remap_note(&channel, note)?;

                if self.mapping_config.is_toggle_note(remapped_note) {
                    self.toggle_note_handler(
                        to_controller_connection,
                        to_software_connection,
                        current_bank,
                        remapped_note,
                        velocity,
                    )?;
                } else {
                    warn!(
                        "Toggle notes doesn't include note (Controller On): {}",
                        u8::from(remapped_note)
                    );
                }
            }

            NoteOff(channel, note, _velocity) => {
                let remapped_note = self.mapping_config.remap_note(&channel, note)?;

                if self.mapping_config.is_toggle_note(remapped_note) {
                    self.led_controller.refresh_single_led(
                        to_controller_connection,
                        &mut self.state_manager,
                        current_bank,
                        remapped_note,
                    )?;
                } else {
                    warn!(
                        "Toggle notes doesn't include note (Controller Off): {}",
                        u8::from(remapped_note)
                    );
                }
            }

            ControlChange(channel, control, velocity) => {
                let remapped_control = self.mapping_config.remap_control(&channel, control)?;
                let message = ControlChange(*current_bank, remapped_control, velocity);
                self.send_midi_message(to_software_connection, message)?;
            }

            _ => {
                self.forward_raw_message(to_software_connection, &midi_message)?;
            }
        }

        Ok(())
    }

    fn process_software_message(
        &mut self,
        midi_message: MidiMessage,
        to_controller_connection: &mut OutputConnection,
    ) -> Result<()> {
        match midi_message {
            NoteOn(channel, note, velocity) => {
                if self.mapping_config.is_toggle_note(note) {
                    self.state_manager.set_note_state(&channel, note, true)?;
                    self.state_manager
                        .set_note_color(&channel, note, velocity)?;

                    self.led_controller.refresh_single_led(
                        to_controller_connection,
                        &mut self.state_manager,
                        &channel,
                        note,
                    )?;
                } else {
                    warn!(
                        "Toggle notes doesn't include note (Software On): {}",
                        u8::from(note)
                    );
                }
            }

            NoteOff(channel, note, _velocity) => {
                if self.mapping_config.is_toggle_note(note) {
                    self.state_manager.set_note_state(&channel, note, false)?;

                    self.led_controller.refresh_single_led(
                        to_controller_connection,
                        &mut self.state_manager,
                        &channel,
                        note,
                    )?;
                } else {
                    warn!(
                        "Toggle notes doesn't include note (Software Off): {}",
                        u8::from(note)
                    );
                }
            }

            ControlChange(_, _, _) => {}
            _ => {}
        }

        Ok(())
    }

    fn toggle_note_handler(
        &mut self,
        to_controller_connection: &mut OutputConnection,
        to_software_connection: &mut OutputConnection,
        bank: &Channel,
        note: Note,
        velocity: Velocity,
    ) -> Result<()> {
        self.state_manager.toggle_note_state(bank, note)?;
        let message = NoteOn(*bank, note, velocity);
        self.send_midi_message(to_software_connection, message)?;

        self.led_controller.refresh_single_led(
            to_controller_connection,
            &mut self.state_manager,
            bank,
            note,
        )?;

        Ok(())
    }

    fn forward_raw_message(
        &self,
        output_connection: &mut OutputConnection,
        message: &MidiMessage,
    ) -> Result<()> {
        self.send_midi_message(output_connection, message.clone())
    }

    fn send_midi_message(
        &self,
        output_connection: &mut OutputConnection,
        message: MidiMessage,
    ) -> Result<()> {
        let self_connection = &mut output_connection.connection;

        if let Some(connection) = self_connection {
            let mut buffer = [0_u8; 3];
            let length = message.copy_to_slice(&mut buffer)?;
            connection.send(&buffer[..length])?;
        }

        Ok(())
    }
}
