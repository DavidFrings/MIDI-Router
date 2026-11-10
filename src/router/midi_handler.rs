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
    pub fn new(config: MappingConfig) -> Self {
        Self {
            state_manager: StateManager::new(),
            mapping_config: config,
            led_controller: LedController::new(),
        }
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

                if self.mapping_config.is_momentary_note(remapped_note) {
                    self.momentary_note_handler(
                        to_controller_connection,
                        to_software_connection,
                        current_bank,
                        NoteOn(*current_bank, remapped_note, velocity),
                    )?;
                } else if self.mapping_config.is_toggle_note(remapped_note) {
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

            NoteOff(channel, note, velocity) => {
                let remapped_note = self.mapping_config.remap_note(&channel, note)?;

                if self.mapping_config.is_momentary_note(remapped_note) {
                    self.momentary_note_handler(
                        to_controller_connection,
                        to_software_connection,
                        current_bank,
                        NoteOff(*current_bank, remapped_note, velocity),
                    )?;
                } else if self.mapping_config.is_toggle_note(remapped_note) {
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
                to_software_connection.send(message)?;
            }

            _ => {
                to_software_connection.send(midi_message)?;
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
        to_software_connection.send(message)?;

        self.led_controller.refresh_single_led(
            to_controller_connection,
            &mut self.state_manager,
            bank,
            note,
        )?;

        Ok(())
    }

    /// Forwards press/release as-is; LED is on only while held.
    fn momentary_note_handler(
        &mut self,
        to_controller_connection: &mut OutputConnection,
        to_software_connection: &mut OutputConnection,
        bank: &Channel,
        message: MidiMessage<'static>,
    ) -> Result<()> {
        let (note, pressed) = match message {
            NoteOn(_, note, _) => (note, true),
            NoteOff(_, note, _) => (note, false),
            _ => return Ok(()),
        };

        self.state_manager.set_note_state(bank, note, pressed)?;
        to_software_connection.send(message)?;

        self.led_controller.refresh_single_led(
            to_controller_connection,
            &mut self.state_manager,
            bank,
            note,
        )?;

        Ok(())
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> MidiHandler {
        let config: MappingConfig = toml::from_str(
            "toggle_notes = [0]\nmomentary_notes = [82]\nnote_map = []\ncontrol_map = []",
        )
        .unwrap();
        MidiHandler::new(config)
    }

    fn is_on(handler: &mut MidiHandler, note: u8) -> bool {
        *handler
            .state_manager
            .get_note_state_and_color(&Channel::Ch1, Note::from_u8_lossy(note))
            .unwrap()
            .0
    }

    fn press(handler: &mut MidiHandler, msg: MidiMessage<'static>) {
        // Unconnected outputs: sends are no-ops, state still updates.
        let (mut to_ctrl, mut to_sw) = (OutputConnection::new(), OutputConnection::new());
        handler.handle_controller_msg(msg, &mut to_ctrl, &mut to_sw).unwrap();
    }

    #[test]
    fn momentary_note_is_on_while_held_and_off_after_release() {
        let mut h = handler();
        let note = Note::from_u8_lossy(82);

        press(&mut h, NoteOn(Channel::Ch1, note, Velocity::MAX));
        assert!(is_on(&mut h, 82));

        press(&mut h, NoteOff(Channel::Ch1, note, Velocity::MAX));
        assert!(!is_on(&mut h, 82));
    }

    #[test]
    fn toggle_note_stays_on_after_release() {
        let mut h = handler();
        let note = Note::from_u8_lossy(0);

        press(&mut h, NoteOn(Channel::Ch1, note, Velocity::MAX));
        press(&mut h, NoteOff(Channel::Ch1, note, Velocity::MAX));
        assert!(is_on(&mut h, 0));
    }
}
