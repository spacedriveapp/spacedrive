# Spacedrive CLI

A comprehensive command-line interface for managing Spacedrive Core with full daemon architecture, real-time monitoring, and cross-device file management.

## Features

- **️ Daemon Architecture**: Background daemon with client-server communication
- **Library Management**: Create, open, switch, and manage multiple libraries
- **Location Management**: Add, remove, and monitor indexed locations with real-time watching
- **️ Job Management**: View, monitor, and control background jobs with live progress
- **Real-time Monitoring**: Beautiful TUI for monitoring job progress and system events
- **Indexing Control**: Start indexing jobs with different modes (shallow/content/deep)
- **Networking Support**: Device pairing, file sharing via Spacedrop
- **Multiple Instances**: Run isolated daemon instances for different use cases
- **Comprehensive Logging**: Built-in logging with file output for debugging
- **️ Cross-platform**: Works on macOS, Linux, and Windows
- **Rich UI**: Colored output, progress bars, and formatted tables

### New Modular Architecture Benefits

The refactored daemon architecture provides:

- **Maintainability**: Each domain (library, location, job, etc.) is isolated in its own handler module
- **Extensibility**: New commands can be added by simply creating a new handler
- **Type Safety**: All commands and responses are strongly typed
- **Code Organization**: Clear separation between command handling, business logic, and transport
- **Testability**: Individual handlers can be unit tested in isolation
- **Performance**: Efficient command routing through handler registry

## Installation

```bash
# Build the CLI
cargo build --release --package sd-cli --package sd-core

# Or build just the CLI for development
cd apps/cli && cargo build

# Install globally (optional)
cargo install --path apps/cli
```

Also you may create an alias to the cli as `sd` or `spacedrive`.

Example for `sd`:
```bash
# For release build
echo 'alias sd="/path/to/spacedrive/target/release/sd-cli"' >> ~/.zshrc

# For debug build (faster compilation)
echo 'alias sd="/path/to/spacedrive/target/debug/sd-cli"' >> ~/.zshrc

# Reload your shell configuration
source ~/.zshrc
```

## Quick Start

```bash
# Start the Spacedrive daemon
sd start

# Create a library and add a location
sd library create "Personal"
sd location add ~/Desktop --name "Desktop"

# Monitor indexing progress
sd job monitor

# Check system status
sd status
```

**Note**: If you haven't set up the alias, use `./target/debug/sd-cli` or `./target/release/sd-cli` instead of `sd`.

## Usage

### Daemon Management

```bash
# Start daemon in background
sd start

# Start daemon with networking enabled
sd start

# Start daemon in foreground (for debugging)
sd start --foreground

# Stop the daemon
sd stop

# Check daemon status
sd status

# Advanced daemon commands
sd daemon status      # Detailed daemon status
sd daemon list        # List all daemon instances
```

### Multiple Daemon Instances

The CLI supports running multiple isolated daemon instances:

```bash
# Run a separate daemon instance
sd --instance test start
sd --instance test library create "Test Library"

# Stop specific instance
sd --instance test stop

# List all running instances
sd daemon list
```

### Basic Commands

```bash
# Show help
sd --help

# Enable verbose logging
sd -v <command>

# Use custom data directory
sd --data-dir /path/to/data <command>
```

### Library Management

```bash
# Create a new library
sd library create "My Library"
sd library create "My Library" --path /custom/path

# List all libraries
sd library list

# Open an existing library
sd library open /path/to/library

# Switch to a library by name or ID
sd library switch "My Library"
sd library switch 12345678-1234-1234-1234-123456789012

# Show current library
sd library current

# Close current library
sd library close
```

### Location Management

```bash
# Add a location to index (automatically starts watching)
sd location add ~/Documents
sd location add ~/Pictures --name "My Photos" --mode deep

# List all locations with status
sd location list

# Remove a location (stops watching and indexing)
sd location remove <location-id>

# Rescan a location (triggers re-indexing)
sd location rescan <location-id>
sd location rescan <location-id> --force  # Full rescan, ignore change detection
```

**Note**: Location IDs are UUIDs displayed in the list command. All location operations work with the daemon automatically.

### Enhanced Indexing

The new indexing system supports different scopes and persistence modes:

```bash
# Quick scan of current directory only (no subdirectories)
sd index quick-scan /path/to/directory --scope current

# Quick scan with ephemeral mode (no database writes)
sd index quick-scan /path/to/directory --scope current --ephemeral

# Browse external paths without adding to managed locations
sd index browse /media/external-drive --scope current
sd index browse /network/drive --scope recursive --content

# Index managed locations with specific scope
sd index location /managed/location --scope current --mode shallow
sd index location <location-uuid> --scope recursive --mode deep

# Legacy full location indexing (backward compatibility)
sd scan /path/to/directory --mode content --watch
```

**Index Scopes:**
- `current`: Index only the specified directory (single level)
- `recursive`: Index the directory and all subdirectories

**Index Modes:**
- `shallow`: Metadata only (fastest)
- `content`: Metadata + content hashing (moderate)
- `deep`: Full analysis including media metadata (slowest)

**Use Cases:**
- **UI Navigation**: `quick-scan --scope current` for instant directory viewing
- **External Browsing**: `browse --ephemeral` for exploring non-managed paths
- **Location Updates**: `location --scope current` to refresh specific directories

### Job Management

```bash
# List all jobs with colored status and progress
sd job list
sd job list --status running      # Filter by status

# Show detailed job information
sd job info <job-id>

# Monitor jobs in real-time with live progress bars
sd job monitor
sd job monitor --job-id <job-id>  # Monitor specific job

# Control jobs (planned features)
sd job pause <job-id>
sd job resume <job-id>
sd job cancel <job-id>
```

**Job Monitor Features:**
- Live progress bars for running jobs
- Color-coded status (running: yellow, completed: green, failed: red)
- ️ Real-time updates every second
- Automatic cleanup of completed jobs
- ️ Ctrl+C to exit gracefully

### File Operations

```bash
# Copy files with progress tracking
sd file copy ~/source.txt ~/destination.txt
sd file copy ~/Photos/*.jpg ~/Backup/ --verify

# Move files
sd file move ~/Downloads/*.pdf ~/Documents/ --preserve-timestamps

# Advanced copy options
sd file copy ~/Project/ ~/Backup/Project/ \
  --overwrite \
  --verify \
  --preserve-timestamps
```

### System Commands

```bash
# Show system status
sd status

# Monitor all system activity (TUI)
sd monitor

# View daemon logs
sd system logs
sd system logs --tail 50
```

### Networking & Device Management

```bash
# Initialize networking (if daemon wasn't started with)
sd network init

# Start/stop networking
sd network start
sd network stop

# List connected devices
sd network devices

# Device pairing
sd network pair --initiate              # Generate pairing code
sd network pair --join <code>           # Join using code
sd network pair --status                # Check pairing status

# Spacedrop (file sharing)
sd network spacedrop <device-id> /path/to/file --sender "Your Name"

# Remove paired device
sd network revoke <device-id>
```

## Real-time Job Monitor

The job monitor provides live progress tracking with beautiful visual indicators:

```bash
sd job monitor
```

### Monitor Features

- **Multi-job tracking**: Monitor all running jobs simultaneously
- **Progress bars**: Visual progress indicators with percentage
- **Color coding**: Status-based colors (yellow=running, green=completed, red=failed)
- **Real-time updates**: Updates every second with latest progress
- **Smart cleanup**: Completed jobs automatically marked and removed
- **Job filtering**: Option to monitor specific jobs

### Sample Output

```
Spacedrive Job Monitor - Press Ctrl+C to exit
═══════════════════════════════════════════

⠚ Indexing Desktop [fdbe777d] [████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 25% | Status: Running
⠂ Indexing Photos [a1b2c3d4]  [██████████████████████████████████████████] 100% | Completed
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
sd start

# 2. Create a library
sd library create "Personal"

# 3. Add locations with different index modes
sd location add ~/Desktop --name "Desktop" --mode content
sd location add ~/Documents --name "Documents" --mode content
sd location add ~/Pictures --name "Photos" --mode deep

# 4. Monitor the indexing progress
sd job monitor

# 5. Check the results
sd location list
sd job list
```

### Multiple Libraries

```bash
# Work with multiple libraries
sd library create "Work"
sd library create "Personal"
sd library list
sd library switch "Work"
sd location add ~/Work/Projects
```

### Batch Indexing

```bash
# Index multiple locations
sd location add ~/Documents --name "Docs"
sd location add ~/Pictures --name "Photos" --mode deep
sd location add ~/Downloads --name "Downloads" --mode shallow
sd job list --status running
```

## Architecture
