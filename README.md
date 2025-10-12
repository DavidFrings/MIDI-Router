# MIDI-Router

MIDI-Router is a Rust-based app that routes MIDI signals between devices and software. It supports multiple controller pages, flexible note remapping, and a REST API for dynamic note changes — all with high performance, low latency, and configurability.

## Features

- High Performance - Built in Rust for maximum efficiency and low latency
- Multiple Pages - Create multiple pages/banks on your controller
- Flexible MIDI Routing - Route MIDI signals between any devices and software
- Configurable - Config file for easy customization
- Auto-Update - Built-in update mechanism via GitHub releases
- REST API (Coming Soon) - Control notes via REST API

## Installation

Download the latest release from the [GitHub Releases](https://github.com/DavidFrings/MIDI-Router/releases) page:

- `midi-router-installer.exe`

## System Requirements

- Windows (primary platform)
- MIDI controller
- Target MIDI software (e.g. Daslight, etc.)

## Building from Source

### Prerequisites

- Rust 1.87 or higher

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/DavidFrings/MIDI-Router.git
cd MIDI-Router

# Build the project
cargo build --release

# Set custom config
copy example.config.toml config.toml
# Now edit config.toml
# (Don't forget to set dev = true on top of the file)
copy config.toml ./target/release/config.toml

# Run the application
cargo run --release
```

The compiled executable will be available in `target/release/`.

## Roadmap

- [x] Installer (`.exe` or `.msi`)
- [x] Configuration file support
- [x] Ratatui implementation
- [ ] REST API for controlling Daslight
- [x] Additional controller support
- Additional controler template configs

## Development Status

**Under Active Development** - This project is currently in development. Features and APIs may change.

## License

This project is licensed under the Mozilla Public License 2.0 (MPL-2.0). See the LICENSE file for details.

## Author

**David Frings**
<!-- - Website: [DavidFrings.dev](https://DavidFrings.dev) -->
- Email: dev@davidfrings.dev

## Contributing

This project is currently under active development. Contributions, issues, and feature requests are welcome!
