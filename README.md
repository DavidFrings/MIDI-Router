# MIDI-Router

A high-performance MIDI routing application written in Rust that efficiently routes MIDI signals between various devices and software applications. Specifically optimized for the AKAI APC40 MkII controller with support for multiple pages/banks.

## Features

- High Performance - Built in Rust for maximum efficiency and low latency
- APC40 MkII Optimization - Specifically designed and optimized for the AKAI APC40 MkII controller (note remaps)
- Multiple Pages - Create multiple pages/banks on your controller
- Flexible MIDI Routing - Route MIDI signals between any devices and software
- Configurable - Command-line arguments for easy customization
- Auto-Update - Built-in update mechanism via GitHub releases
- REST API (Coming Soon) - Control Daslight via REST API

## Installation

Download the latest release from the [GitHub Releases](https://github.com/DavidFrings/MIDI-Router/releases) page:

- `apc40mk2-router.exe` - Main application
- `updater.exe` - Auto-updater application
- `installer.exe` or `.msi` - Installer (coming soon)

Extract the files to a directory of your choice and run `apc40mk2-router.exe`.

## Usage

### Basic Usage

Run the application with default settings (APC40 mkII → Daslight):

```bash
apc40mk2-router.exe
```

### Command Line Arguments

```bash
apc40mk2-router.exe [OPTIONS]
```

**Options:**

- `-c,  --controller-name <NAME>` - MIDI controller port name (default: "APC40 mkII")
- `-s,  --software-name <NAME>` - Target software MIDI port name (default: "Daslight")
- `-nc, --no-checks` - No Checks for other software (e.g. loopMIDI) 
- `-h,  --help` - Print help information
- `-V,  --version` - Print version information

### Environment Variables

Control logging verbosity using the `RUST_LOG` environment variable:

```bash
set RUST_LOG=debug
apc40mk2-router.exe
```

Available log levels: `error`, `warn`, `info` (default), `debug`, `trace`

### Examples

Route from APC40 mkII to Daslight (default):
```bash
apc40mk2-router.exe
```

Route from a custom controller to Daslight:
```bash
apc40mk2-router.exe --controller-name "My MIDI Controller"
```

Route to different software:
```bash
apc40mk2-router.exe --controller-name "APC40 mkII" --software-name "Daslight 5"
```

Enable debug logging:
```bash
set RUST_LOG=debug
apc40mk2-router.exe
```

## System Requirements

- Windows (primary platform)
- MIDI controller (optimized for AKAI APC40 MkII)
- Target MIDI software (e.g., Daslight, etc.)

## Building from Source

### Prerequisites

- Rust 1.87 or higher
- Cargo (comes with Rust)

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/DavidFrings/MIDI-Router.git
cd MIDI-Router

# Build the project
cargo build --release

# Run the application
cargo run --release
```

The compiled executable will be available in `target/release/`.

## Roadmap

- [x] Installer (`.exe` or `.msi`)
- [x] Configuration file support
- [x] Ratatui implementation
- [ ] REST API for controlling Daslight
- [ ] Additional controller support
- [ ] Check for LoopMIDI running (disablable with arguments)

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

## Acknowledgments

- Built with [midir](https://github.com/Boddlnagg/midir) for cross-platform MIDI support
- Optimized for the AKAI APC40 MkII controller
