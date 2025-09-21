# Spacedrive SwiftUI Companion App

A lightweight, native macOS companion application for monitoring Spacedrive daemon jobs in real-time.

## Overview

This SwiftUI app provides a translucent, always-on-top window that displays live updates of jobs running in the Spacedrive daemon. It connects directly to the daemon via Unix domain socket and subscribes to job events for real-time updates.

## Features

- **Translucent Window**: Blends seamlessly with the macOS desktop environment
- **Real-time Job Monitoring**: Live updates via daemon event subscription
- **Job Status Tracking**: Visual indicators for running, completed, failed, and paused jobs
- **Progress Visualization**: Progress bars for active jobs
- **Connection Status**: Visual indicator of daemon connection health
- **Minimal Resource Usage**: Lightweight companion app design

## Requirements

- macOS 13.0 or later
- Running Spacedrive daemon instance
- Swift 5.9 or later

## Building and Running

### Using Swift Package Manager

```bash
cd /path/to/spacedrive/apps/spacedrive-companion
swift build
swift run
```

### Using Xcode

1. Open the `Package.swift` file in Xcode
2. Build and run the project (⌘+R)

## Architecture

The app follows a clean MVVM architecture:

- **Models** (`JobModels.swift`): Codable structs for job data and RPC communication
- **Views**:
  - `ContentView.swift`: Main app view
  - `JobMonitorView.swift`: Job list container
  - `JobRowView.swift`: Individual job display
- **ViewModel** (`JobListViewModel.swift`): State management and data binding
- **Services** (`DaemonConnector.swift`): Unix socket communication with daemon
- **UI Components** (`TranslucentWindow.swift`): Custom translucent window implementation

## Daemon Communication

The app communicates with the Spacedrive daemon via:

1. **Unix Domain Socket**: `~/.local/share/spacedrive/daemon/daemon.sock`
2. **RPC Protocol**: JSON-based request/response over the socket
3. **Event Subscription**: Real-time job event stream
4. **Initial State**: `jobs.list` query on connection

### Supported Events

- `JobStarted`: New job initiated
- `JobProgress`: Job progress updates
- `JobCompleted`: Job finished successfully
- `JobFailed`: Job encountered an error
- `JobPaused`: Job was paused

## Usage

1. Ensure the Spacedrive daemon is running
2. Launch the companion app
3. The app will automatically connect and display job status
4. Use the refresh button to reconnect if needed

## Future Enhancements

- Job control buttons (pause/resume/cancel)
- System tray integration
- Push notifications for job completion
- Multiple library support
- Job filtering and search

## Development Notes

This is a Proof of Concept implementation focusing on read-only job monitoring. The architecture is designed to easily accommodate future interactive features.

The app uses modern SwiftUI patterns and follows macOS design guidelines for system utility applications.


