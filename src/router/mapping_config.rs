use anyhow::Result;
use serde::{Deserialize, Serialize};
use wmidi::{Channel, ControlFunction, Note, U7};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappingConfig {
    toggle_notes: Vec<u8>,
    #[serde(default)]
    momentary_notes: Vec<u8>,
    note_map: Vec<NoteMap>,
    control_map: Vec<ControlMap>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NoteMap {
    note: u8,
    new_note: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ControlMap {
    note: u8,
    new_note: Vec<u8>,
}

impl MappingConfig {
    pub fn new(config: MappingConfig) -> Self {
        config
    }

    pub fn is_toggle_note(&self, conn_note: Note) -> bool {
        self.toggle_notes.contains(&u8::from(conn_note))
    }

    pub fn is_momentary_note(&self, conn_note: Note) -> bool {
        self.momentary_notes.contains(&u8::from(conn_note))
    }

    pub fn get_toggle_notes(&self) -> &Vec<u8> {
        &self.toggle_notes
    }

    pub fn remap_note(&self, channel: &Channel, conn_note: Note) -> Result<Note> {
        for map in &self.note_map {
            if map.note == u8::from(conn_note) {
                return if let Some(&new_note) = map.new_note.get(channel.index() as usize) {
                    Ok(Note::from_u8_lossy(new_note))
                } else {
                    Ok(conn_note)
                };
            }
        }

        Ok(conn_note)
    }

    pub fn remap_control(
        &self,
        channel: &Channel,
        conn_control: ControlFunction,
    ) -> Result<ControlFunction> {
        for map in &self.control_map {
            if map.note == u8::from(conn_control) {
                return if let Some(&new_control) = map.new_note.get(channel.index() as usize) {
                    Ok(ControlFunction::from(U7::from_u8_lossy(new_control)))
                } else {
                    Ok(conn_control)
                };
            }
        }

        Ok(conn_control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MappingConfig {
        toml::from_str(
            r#"
            toggle_notes = [0, 1, 40, 41]

            [[note_map]]
            note = 48
            new_note = [40, 41, 42, 43, 44, 45, 46, 47]

            [[control_map]]
            note = 7
            new_note = [0, 1, 2, 3, 4, 5, 9, 10]
            "#,
        )
        .unwrap()
    }

    fn cc(n: u8) -> ControlFunction {
        ControlFunction::from(U7::from_u8_lossy(n))
    }

    #[test]
    fn track_fader_is_remapped_per_channel_via_control_map() {
        let remapped = config().remap_control(&Channel::Ch2, cc(7)).unwrap();
        assert_eq!(u8::from(remapped), 1);
    }

    #[test]
    fn track_knob_cc_matching_a_note_map_key_is_not_remapped() {
        let remapped = config().remap_control(&Channel::Ch2, cc(48)).unwrap();
        assert_eq!(u8::from(remapped), 48);
    }

    #[test]
    fn record_arm_note_is_remapped_per_channel() {
        let remapped = config().remap_note(&Channel::Ch2, Note::from_u8_lossy(48)).unwrap();
        assert_eq!(u8::from(remapped), 41);
    }

    #[test]
    fn momentary_notes_is_optional_in_config() {
        assert!(!config().is_momentary_note(Note::from_u8_lossy(82)));
    }

    #[test]
    fn listed_momentary_note_is_detected() {
        let config: MappingConfig = toml::from_str(
            "toggle_notes = []\nmomentary_notes = [82]\nnote_map = []\ncontrol_map = []",
        )
        .unwrap();
        assert!(config.is_momentary_note(Note::from_u8_lossy(82)));
    }
}
