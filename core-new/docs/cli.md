# Spacedrive CLI

A comprehensive command-line interface for managing Spacedrive Core with full daemon architecture, real-time monitoring, and cross-device file management.

## Features

- **🏗️ Daemon Architecture**: Background daemon with client-server communication
- **📚 Library Management**: Create, open, switch, and manage multiple libraries
- **📁 Location Management**: Add, remove, and monitor indexed locations with real-time watching
- **⚙️ Job Management**: View, monitor, and control background jobs with live progress
- **📊 Real-time Monitoring**: Beautiful TUI for monitoring job progress and system events
- **🔍 Indexing Control**: Start indexing jobs with different modes (shallow/content/deep)
- **🖥️ Cross-platform**: Works on macOS, Linux, and Windows
- **🎨 Rich UI**: Colored output, progress bars, and formatted tables

## Installation

```bash
# Build the CLI
cargo build --release --bin spacedrive

# Install globally (optional)
cargo install --path . --bin spacedrive
```

## Quick Start

```bash
# Start the Spacedrive daemon
spacedrive start

# Create a library and add a location
spacedrive library create "Personal" 
spacedrive location add ~/Desktop --name "Desktop"

# Monitor indexing progress
spacedrive job monitor

# Check system status
spacedrive status
```

## Usage

### Daemon Management

```bash
# Start daemon in background
spacedrive start

# Start daemon in foreground (for debugging)
spacedrive start --foreground

# Stop the daemon
spacedrive stop

# Check daemon status
spacedrive daemon
```

### Basic Commands

```bash
# Show help
spacedrive --help

# Enable verbose logging
spacedrive -v <command>

# Use custom data directory
spacedrive --data-dir /path/to/data <command>
```

### Library Management

```bash
# Create a new library
spacedrive library create "My Library"
spacedrive library create "My Library" --path /custom/path

# List all libraries
spacedrive library list

# Open an existing library
spacedrive library open /path/to/library

# Switch to a library by name or ID
spacedrive library switch "My Library"
spacedrive library switch 12345678-1234-1234-1234-123456789012

# Show current library
spacedrive library current

# Close current library
spacedrive library close
```

### Location Management

```bash
# Add a location to index (automatically starts watching)
spacedrive location add ~/Documents
spacedrive location add ~/Pictures --name "My Photos" --mode deep

# List all locations with status
spacedrive location list

# Remove a location (stops watching and indexing)
spacedrive location remove <location-id>

# Rescan a location (triggers re-indexing)
spacedrive location rescan <location-id>
spacedrive location rescan <location-id> --force  # Full rescan, ignore change detection
```

**Note**: Location IDs are UUIDs displayed in the list command. All location operations work with the daemon automatically.

### Indexing

```bash
# Start indexing with default settings (content mode)
spacedrive index ~/Desktop

# Index with specific mode
spacedrive index ~/Videos --mode shallow  # Metadata only
spacedrive index ~/Photos --mode deep     # Full analysis

# Index and watch progress
spacedrive index ~/Documents --watch
```

### Job Management

```bash
# List all jobs with colored status and progress
spacedrive job list
spacedrive job list --status running      # Filter by status

# Show detailed job information
spacedrive job info <job-id>

# Monitor jobs in real-time with live progress bars
spacedrive job monitor
spacedrive job monitor --job-id <job-id>  # Monitor specific job

# Control jobs (planned features)
spacedrive job pause <job-id>
spacedrive job resume <job-id>
spacedrive job cancel <job-id>
```

**Job Monitor Features:**
- 🔴 Live progress bars for running jobs
- 🎨 Color-coded status (running: yellow, completed: green, failed: red)
- ⏱️ Real-time updates every second
- 🧹 Automatic cleanup of completed jobs
- ⌨️ Ctrl+C to exit gracefully

### System Commands

```bash
# Show system status
spacedrive status

# Monitor all system activity (TUI)
spacedrive monitor
```

## Real-time Job Monitor

The job monitor provides live progress tracking with beautiful visual indicators:

```bash
spacedrive job monitor
```

### Monitor Features

- **🎯 Multi-job tracking**: Monitor all running jobs simultaneously
- **📊 Progress bars**: Visual progress indicators with percentage
- **🎨 Color coding**: Status-based colors (yellow=running, green=completed, red=failed)
- **⚡ Real-time updates**: Updates every second with latest progress
- **🧹 Smart cleanup**: Completed jobs automatically marked and removed
- **🔍 Job filtering**: Option to monitor specific jobs

### Sample Output

```
📡 Spacedrive Job Monitor - Press Ctrl+C to exit
═══════════════════════════════════════════

⠚ Indexing Desktop [fdbe777d] [████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 25% | Status: Running
⠂ Indexing Photos [a1b2c3d4]  [██████████████████████████████████████████] 100% | ✅ Completed
⠈ Content Analysis [e5f6g7h8] [██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 15% | Status: Running
```

## Indexing Modes

- **Shallow**: Fast metadata-only indexing (file names, sizes, dates)
- **Content**: Standard indexing with content hashing for deduplication
- **Deep**: Comprehensive analysis including media metadata extraction

## Examples

### Complete Workflow

```bash
# 1. Start the daemon
spacedrive start

# 2. Create a library
spacedrive library create "Personal"

# 3. Add locations with different index modes
spacedrive location add ~/Desktop --name "Desktop" --mode content
spacedrive location add ~/Documents --name "Documents" --mode content  
spacedrive location add ~/Pictures --name "Photos" --mode deep

# 4. Monitor the indexing progress
spacedrive job monitor

# 5. Check the results
spacedrive location list
spacedrive job list
```

### Multiple Libraries

```bash
# Work with multiple libraries
spacedrive library create "Work"
spacedrive library create "Personal"
spacedrive library list
spacedrive library switch "Work"
spacedrive location add ~/Work/Projects
```

### Batch Indexing

```bash
# Index multiple locations
spacedrive location add ~/Documents --name "Docs"
spacedrive location add ~/Pictures --name "Photos" --mode deep
spacedrive location add ~/Downloads --name "Downloads" --mode shallow
spacedrive job list --status running
```

## Architecture

### Daemon-Client Model

The Spacedrive CLI uses a daemon-client architecture for optimal performance:

```
┌─────────────────┐    Unix Socket    ┌─────────────────┐
│   CLI Client    │ ◄─────────────► │  Spacedrive     │
│   (Commands)    │                  │  Daemon         │
└─────────────────┘                  └─────────────────┘
                                            │
                                            ▼
                                    ┌─────────────────┐
                                    │  Background     │
                                    │  Jobs, Watching │
                                    │  & File System  │
                                    └─────────────────┘
```

**Benefits:**
- 🚀 **Fast responses**: Daemon keeps state in memory
- 🔄 **Background processing**: Jobs continue when CLI exits
- 📡 **Real-time updates**: File system changes processed immediately
- 💾 **Persistent state**: Libraries and locations survive restarts

### Configuration

The daemon stores data in the specified data directory:

```
spacedrive-cli-data/
├── libraries/           # Library database files
├── daemon.sock         # Unix socket for communication
├── daemon.pid          # Process ID file
└── cli_state.json      # CLI preferences and history
```

## Tips & Best Practices

1. **🔧 Start Daemon First**: Always run `spacedrive start` before other commands
2. **📊 Monitor Progress**: Use `spacedrive job monitor` for real-time feedback on indexing
3. **💻 Use Verbose Mode**: Add `-v` flag for detailed logging during troubleshooting
4. **🚀 Start Simple**: Begin with shallow indexing for quick results, then upgrade to deeper modes
5. **📁 Organize by Purpose**: Create separate libraries for different use cases (Work, Personal, etc.)
6. **⚡ Daemon Persistence**: The daemon keeps running in the background - jobs continue even if you close the terminal

## Troubleshooting

### Daemon Issues
```bash
# Check if daemon is running
spacedrive daemon

# Restart daemon
spacedrive stop
spacedrive start

# Run daemon in foreground for debugging
spacedrive start --foreground -v
```

### Communication Errors
```bash
# Check daemon status
spacedrive daemon

# Look for socket file
ls -la spacedrive-cli-data/daemon.sock

# Restart daemon if socket is missing
spacedrive stop && spacedrive start
```

### Job Issues
```bash
# Check job status with details
spacedrive job list -v

# View specific job information
spacedrive job info <job-id>

# Monitor jobs in real-time
spacedrive job monitor
```

### Location Issues
```bash
# Check location status
spacedrive location list

# Force rescan if files not updating
spacedrive location rescan <location-id> --force

# Remove and re-add problematic locations
spacedrive location remove <location-id>
spacedrive location add /path/to/location
```

### Performance Issues
```bash
# Use shallow mode for large directories
spacedrive location add /large/directory --mode shallow

# Check system status
spacedrive status

# Monitor daemon resources with verbose output
spacedrive start --foreground -v
```