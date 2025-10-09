use anyhow::Result;
use wmidi::{Channel, ControlFunction, Note, U7};

pub struct MappingConfig {
    toggle_notes: Vec<u8>,
    note_map: Vec<(u8, Vec<u8>)>,
    control_map: Vec<(u8, Vec<u8>)>,
}

impl MappingConfig {
    pub fn new() -> Self {
        Self {
            toggle_notes: vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
                44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
            ],
            note_map: vec![
                (48, vec![40, 41, 42, 43, 44, 45, 46, 47]),
                (49, vec![68, 69, 70, 71, 72, 73, 74, 75]),
                (50, vec![53, 54, 55, 56, 76, 77, 78, 79]),
                (52, vec![105, 106, 107, 108, 109, 110, 111, 112]),
                (66, vec![113, 114, 115, 116, 117, 118, 119, 120]),
            ],
            control_map: vec![(7, vec![0, 1, 2, 3, 4, 5, 9, 10])],
        }
    }

    pub fn is_toggle_note(&self, conn_note: Note) -> bool {
        self.toggle_notes.contains(&u8::from(conn_note))
    }

    pub fn get_toggle_notes(&self) -> &Vec<u8> {
        &self.toggle_notes
    }

    pub fn remap_note(&self, channel: &Channel, conn_note: Note) -> Result<Note> {
        for (original_note, remapped_notes) in &self.note_map {
            if *original_note == u8::from(conn_note) {
                if (channel.index() as usize) < remapped_notes.len() {
                    return Ok(Note::from_u8_lossy(
                        remapped_notes[channel.index() as usize],
                    ));
                }
            }
        }

        Ok(conn_note)
    }

    pub fn remap_control(
        &self,
        channel: &Channel,
        conn_control: ControlFunction,
    ) -> Result<ControlFunction> {
        for (original_control, remapped_controls) in &self.control_map {
            if *original_control == u8::from(conn_control) {
                if (channel.index() as usize) < remapped_controls.len() {
                    return Ok(ControlFunction::from(U7::from_u8_lossy(
                        remapped_controls[channel.index() as usize],
                    )));
                }
            }
        }

        Ok(conn_control)
    }
}
