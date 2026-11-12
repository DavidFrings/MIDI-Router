use crate::router::mapping_config::MappingConfig;
use crate::router::{
    input_connection::{
        InputConnection,
        InputMessage::{ControllerMessage, SoftwareMessage},
    },
    midi_handler::MidiHandler,
    output_connection::OutputConnection,
    remote::{RemoteEvent, RouterHandles},
};
use anyhow::{Context, Result, anyhow};
use log::info;
use midir::{MidiIO, MidiInput, MidiInputPort, MidiOutput, MidiOutputPort};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

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
    pub fn new(config: MappingConfig, events: broadcast::Sender<RemoteEvent>) -> Self {
        let mut handler = MidiHandler::new(config);
        handler.set_event_sender(events);

        Self {
            from_controller_connection: InputConnection::new(),
            to_controller_connection: Arc::new(Mutex::new(OutputConnection::new())),
            from_software_connection: InputConnection::new(),
            to_software_connection: Arc::new(Mutex::new(OutputConnection::new())),
            midi_handler: Arc::new(Mutex::new(handler)),
        }
    }

    pub fn handles(&self) -> RouterHandles {
        RouterHandles {
            handler: Arc::clone(&self.midi_handler),
            to_controller: Arc::clone(&self.to_controller_connection),
            to_software: Arc::clone(&self.to_software_connection),
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

        /// Find the port whose name contains `name`; never fall back to another device.
        fn find_port<P: MidiIO>(midi_io: &P, ports: &[P::Port], name: &str) -> Result<P::Port> {
            ports
                .iter()
                .find(|port| {
                    midi_io
                        .port_name(port)
                        .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                        .unwrap_or(false)
                })
                .cloned()
                .ok_or_else(|| anyhow!("Could not find a MIDI device containing '{}'", name))
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
}
