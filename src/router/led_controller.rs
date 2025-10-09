use crate::router::{output_connection::OutputConnection, state_manager::StateManager};
use anyhow::Result;
use std::{thread, time::Duration};
use wmidi::{Channel, MidiMessage, MidiMessage::NoteOn, Note, Velocity};

pub struct LedController;

impl LedController {
    pub fn new() -> Self {
        Self
    }

    pub fn refresh_all_leds(
        &self,
        to_controller_connection: &mut OutputConnection,
        state_manager: &mut StateManager,
        bank: &Channel,
        toggle_notes: &[u8],
    ) -> Result<()> {
        for &note_u8 in toggle_notes {
            let note = Note::from_u8_lossy(note_u8);
            self.refresh_single_led(to_controller_connection, state_manager, bank, note)?;
        }

        Ok(())
    }

    pub fn refresh_single_led(
        &self,
        to_controller_connection: &mut OutputConnection,
        state_manager: &mut StateManager,
        bank: &Channel,
        note: Note,
    ) -> Result<()> {
        let (state, color) = state_manager.get_note_state_and_color(bank, note)?;

        let velocity = Velocity::from_u8_lossy(*color);

        let message = if *state {
            NoteOn(Channel::Ch13, note, velocity) // When Note is ON, use Channel 13 for LED ON (Blinking)
        } else {
            NoteOn(Channel::Ch1, note, velocity)
        };

        self.send_led_message(to_controller_connection, message)?;

        thread::sleep(Duration::from_micros(25)); // Small delay to ensure the port is not overwhelmed

        Ok(())
    }

    fn send_led_message(
        &self,
        output_connection: &mut OutputConnection,
        msg: MidiMessage,
    ) -> Result<()> {
        let connection = &mut output_connection.connection;

        if let Some(conn) = connection {
            let mut buffer = [0_u8; 3];
            let length = msg.copy_to_slice(&mut buffer)?;
            conn.send(&buffer[..length])?;
        }

        Ok(())
    }
}
