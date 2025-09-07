# Spacedrive Core CLI

A comprehensive command-line interface for managing Spacedrive libraries, locations, indexing operations, and jobs.

## Features

- **Library Management**: Create, open, switch, and manage Spacedrive libraries
- **Location Management**: Add, remove, and rescan file system locations
- **Indexing Operations**: Control indexing with start, pause, resume, and status commands
- **Job Management**: Monitor and control background jobs with real-time progress
- **Event Monitoring**: Watch system events in real-time with filtering
- **Interactive TUI**: Full-featured terminal user interface for interactive management

## Installation

```bash
# Build the CLI
cargo build --release --bin spacedrive-cli

# Or run directly
cargo run --bin spacedrive-cli -- --help
```

## Usage

### Basic Commands

```bash
# Create a new library
spacedrive-cli library create "My Library"

# Open an existing library
spacedrive-cli library open /path/to/library.sdlibrary

# List all libraries
spacedrive-cli library list --detailed

# Add a location
spacedrive-cli location add /path/to/folder --name "Documents" --mode full

# Start indexing
spacedrive-cli index start

# Monitor jobs
spacedrive-cli job monitor
```

### Interactive TUI Mode

Launch the interactive terminal UI:

```bash
spacedrive-cli tui
```

In TUI mode:
- **Tab**: Switch between tabs (Overview, Locations, Jobs, Events)
- **↑/↓**: Navigate lists
- **Enter**: Select items
- **q**: Quit

### Watch Events

Monitor system events in real-time:

```bash
# Watch all events
spacedrive-cli watch

# Watch specific event types
spacedrive-cli watch --filter job
spacedrive-cli watch --filter library
spacedrive-cli watch --filter indexing
```

## Command Reference

### Library Commands

- `library create <name>` - Create a new library
- `library open <path>` - Open an existing library
- `library list` - List all libraries
- `library switch <id/name>` - Switch to a different library
- `library info` - Show current library information
- `library close` - Close the current library
- `library delete <id>` - Delete a library

### Location Commands

- `location add <path>` - Add a new location
- `location remove <id>` - Remove a location
- `location list` - List all locations
- `location rescan [id]` - Rescan location(s)

### Index Commands

- `index start [location]` - Start indexing
- `index pause [job]` - Pause indexing
- `index resume [job]` - Resume indexing
- `index status` - Show indexing status
- `index stats [location]` - Show indexing statistics

### Job Commands

- `job list` - List all jobs
- `job info <id>` - Show job information
- `job cancel <id>` - Cancel a job
- `job clear` - Clear completed jobs
- `job monitor [id]` - Monitor job progress

## Environment Variables

- `SPACEDRIVE_LIBRARY_DIR` - Set custom library directory (default: ~/Spacedrive/Libraries)

## Examples

### Create and Index a Library

```bash
# Create a new library
spacedrive-cli library create "Photos"

# Add locations
spacedrive-cli location add ~/Pictures --name "My Pictures" --mode full
spacedrive-cli location add /mnt/external/photos --name "External Photos" --mode content

# Start indexing all locations
spacedrive-cli index start

# Monitor progress
spacedrive-cli job monitor
```

### Batch Operations

```bash
# Create library with custom location
spacedrive-cli library create "Work" --location ~/Work/Libraries

# Add multiple locations
for dir in ~/Documents ~/Projects ~/Downloads; do
  spacedrive-cli location add "$dir" --mode quick
done

# Watch indexing progress
spacedrive-cli watch --filter indexing
```

## Architecture

The CLI is built with:
- **clap**: Command-line argument parsing
- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal manipulation
- **indicatif**: Progress bars and spinners
- **console**: Terminal styling
- **dialoguer**: Interactive prompts

The CLI integrates directly with Spacedrive Core's:
- Event bus for real-time updates
- Job manager for background operations
- Library and location managers
- Database layer for persistence