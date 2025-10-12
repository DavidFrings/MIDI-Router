use anyhow::{Result, format_err};
use std::collections::HashMap;
use wmidi::{Channel, Note, Velocity};

pub struct StateManager {
    states_map: HashMap<u8, Vec<bool>>,
    color_map: HashMap<u8, Vec<u8>>,
    current_bank: Channel,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            states_map: (0..=8)
                .map(|i| (i, vec![false; 128]))
                .collect::<HashMap<_, _>>(),
            color_map: (0..=8)
                .map(|i| (i, vec![0; 128]))
                .collect::<HashMap<_, _>>(),
            current_bank: Channel::Ch1, // ToDo: get from controller
        }
    }

    pub fn _get_states_map(&self) -> &HashMap<u8, Vec<bool>> {
        &self.states_map
    }

    pub fn _get_color_map(&self) -> &HashMap<u8, Vec<u8>> {
        &self.color_map
    }

    pub fn get_current_bank(&self) -> &Channel {
        &self.current_bank
    }

    pub fn set_current_bank(&mut self, channel: Channel) {
        self.current_bank = channel;
    }

    pub fn toggle_note_state(&mut self, bank: &Channel, note: Note) -> Result<()> {
        let states_map = &mut self.states_map;

        if let Some(state) = states_map
            .get_mut(&bank.index())
            .and_then(|states| states.get_mut(u8::from(note) as usize))
        {
            *state = !*state;
            return Ok(());
        }

        Err(format_err!(""))
    }

    pub fn set_note_state(&mut self, bank: &Channel, note: Note, new_state: bool) -> Result<()> {
        let states_map = &mut self.states_map;

        if let Some(state) = states_map
            .get_mut(&bank.index())
            .and_then(|states| states.get_mut(u8::from(note) as usize))
        {
            *state = new_state;
            return Ok(());
        }

        Err(format_err!(""))
    }

    pub fn set_note_color(
        &mut self,
        bank: &Channel,
        note: Note,
        new_color: Velocity,
    ) -> Result<()> {
        let color_map = &mut self.color_map;

        if let Some(color) = color_map
            .get_mut(&bank.index())
            .and_then(|colors| colors.get_mut(u8::from(note) as usize))
        {
            *color = u8::from(new_color);
            return Ok(());
        }

        Err(format_err!(""))
    }

    pub fn get_note_state_and_color(&mut self, bank: &Channel, note: Note) -> Result<(&bool, &u8)> {
        let states_map = &mut self.states_map;
        let color_map = &mut self.color_map;

        if let Some(state) = states_map
            .get(&bank.index())
            .and_then(|states| states.get(u8::from(note) as usize))
        {
            if let Some(color) = color_map
                .get(&bank.index())
                .and_then(|colors| colors.get(u8::from(note) as usize))
            {
                return Ok((state, color));
            }
        }

        Err(format_err!(""))
    }
}
