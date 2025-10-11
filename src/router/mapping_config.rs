use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use wmidi::{Channel, ControlFunction, Note, U7};

#[derive(Debug, Serialize, Deserialize)]
pub struct MappingConfig {
    toggle_notes: Vec<u8>,
    note_map: Vec<NoteMap>,
    control_map: Vec<ControlMap>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NoteMap {
    note: u8,
    new_note: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ControlMap {
    note: u8,
    new_note: Vec<u8>,
}

impl MappingConfig {
    pub fn new() -> Result<Self> {
        if let Ok(data) = fs::read_to_string("config.toml") {
            let config: MappingConfig = toml::from_str(&data)?;
            //let debug_output = format!("{:#?}", config);
            //fs::write("debug.txt", debug_output)?;

            Ok(config)
        } else {
            Err(anyhow!("config.toml not found!"))
        }
    }

    pub fn is_toggle_note(&self, conn_note: Note) -> bool {
        self.toggle_notes.contains(&u8::from(conn_note))
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
        conn_note: ControlFunction,
    ) -> Result<ControlFunction> {
        for map in &self.note_map {
            if map.note == u8::from(conn_note) {
                return if let Some(&new_note) = map.new_note.get(channel.index() as usize) {
                    Ok(ControlFunction::from(U7::from_u8_lossy(new_note)))
                } else {
                    Ok(conn_note)
                };
            }
        }

        Ok(conn_note)
    }
}
