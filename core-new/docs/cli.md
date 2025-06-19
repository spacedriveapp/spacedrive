# Spacedrive CLI

A comprehensive command-line interface for managing Spacedrive Core, including libraries, locations, indexing, and job management.

## Features

- **Library Management**: Create, open, switch, and manage multiple libraries
- **Location Management**: Add, remove, and monitor indexed locations
- **Job Management**: View, monitor, and control background jobs
- **Real-time Monitoring**: Beautiful TUI for monitoring job progress and system events
- **Indexing Control**: Start indexing jobs with different modes
- **Status Overview**: Quick system and library status checks

## Installation

```bash
# Build the CLI
cargo build --release --bin spacedrive

# Install globally (optional)
cargo install --path . --bin spacedrive
```

## Usage

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
# Add a location to index
spacedrive location add ~/Documents
spacedrive location add ~/Pictures --name "My Photos" --mode deep

# List all locations
spacedrive location list

# Get location details
spacedrive location info 1
spacedrive location info ~/Documents

# Remove a location
spacedrive location remove 1

# Rescan a location
spacedrive location rescan 1
spacedrive location rescan 1 --force  # Full rescan, ignore change detection
```

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
# List all jobs
spacedrive job list
spacedrive job list --recent              # Show only recent jobs
spacedrive job list --status running      # Filter by status

# Show job details
spacedrive job info 12345678-1234-1234-1234-123456789012

# Monitor jobs in real-time (TUI)
spacedrive job monitor

# Control jobs
spacedrive job pause <job-id>
spacedrive job resume <job-id>
spacedrive job cancel <job-id>
```

### System Commands

```bash
# Show system status
spacedrive status

# Monitor all system activity (TUI)
spacedrive monitor
```

## Real-time Job Monitor

The job monitor provides a beautiful TUI for monitoring jobs and system events:

```bash
spacedrive job monitor
```

### Monitor Controls

- **↑/↓**: Navigate between jobs
- **q/ESC**: Quit monitor
- **c**: Clear event log

### Monitor Display

The monitor shows:
- **Jobs List**: All jobs with status, progress, and duration
- **Job Details**: Detailed information about the selected job
- **Event Log**: Real-time system events and job updates
- **Progress Bar**: Visual progress indicator for running jobs

## Indexing Modes

- **Shallow**: Fast metadata-only indexing (file names, sizes, dates)
- **Content**: Standard indexing with content hashing for deduplication
- **Deep**: Comprehensive analysis including media metadata extraction

## Examples

### Quick Start

```bash
# Create a library and index your desktop
spacedrive library create "Personal"
spacedrive location add ~/Desktop --mode content
spacedrive job monitor  # Watch the indexing progress
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

## Configuration

The CLI stores its state in `cli_state.json` within the data directory:

```json
{
  "current_library_id": "12345678-1234-1234-1234-123456789012",
  "last_library_path": "/path/to/library",
  "command_history": [...],
  "max_history": 100
}
```

## Tips

1. **Use Tab Completion**: The CLI supports shell completion for commands and arguments
2. **Monitor Long Jobs**: Use `--watch` flag or `job monitor` for long-running operations
3. **Check Status First**: Run `spacedrive status` to ensure everything is working
4. **Start Simple**: Begin with shallow indexing for quick results, then upgrade to deeper modes

## Troubleshooting

### Library Not Found
```bash
# Check if library exists
spacedrive library list

# Re-open library
spacedrive library open /path/to/library
```

### Indexing Issues
```bash
# Check job status
spacedrive job list --recent

# View job details for errors
spacedrive job info <job-id>

# Force rescan if needed
spacedrive location rescan <location-id> --force
```

### Performance
```bash
# Use shallow mode for large directories
spacedrive index /large/directory --mode shallow

# Monitor system resources
spacedrive monitor
```