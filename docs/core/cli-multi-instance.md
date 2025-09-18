# Multi-Instance Daemon Support

Spacedrive CLI now supports running multiple daemon instances simultaneously, enabling local testing of device pairing and other multi-device features.

## Overview

Multiple daemon instances allow you to:
- Test device pairing locally by running two instances
- Simulate multi-device scenarios on a single machine
- Isolate different development/testing environments
- Run production and development daemons side-by-side

## Usage

### Starting Multiple Instances

```bash
# Start default instance
spacedrive start

# Start named instances
spacedrive start --instance alice
spacedrive start --instance bob

# Start with networking enabled
spacedrive start --instance alice --enable-networking
spacedrive start --instance bob --enable-networking
```

### Targeting Specific Instances

Use the `--instance` flag to target commands to specific daemon instances:

```bash
# Default instance
spacedrive library list

# Named instances
spacedrive --instance alice library list
spacedrive --instance bob library create "Bob's Library"
```

### Instance Management

```bash
# List all daemon instances
spacedrive instance list

# Stop specific instance
spacedrive instance stop alice
spacedrive --instance bob stop  # Alternative syntax

# Check status of specific instance
spacedrive --instance alice daemon
```

### Device Pairing Example

Test device pairing locally using two instances:

```bash
# Terminal 1: Start Alice's daemon
spacedrive start --instance alice --enable-networking --foreground

# Terminal 2: Start Bob's daemon
spacedrive start --instance bob --enable-networking --foreground

# Terminal 3: Alice generates pairing code
spacedrive --instance alice network init --password "test123"
spacedrive --instance alice network pair generate --auto-accept

# Terminal 4: Bob joins using Alice's code
spacedrive --instance bob network init --password "test123"
spacedrive --instance bob network pair join "word1 word2 word3 ... word12"
```

## Architecture

### Instance Isolation

Each instance has completely isolated:

- **Socket paths**: `spacedrive.sock`, `spacedrive-alice.sock`, `spacedrive-bob.sock`
- **PID files**: `spacedrive.pid`, `spacedrive-alice.pid`, `spacedrive-bob.pid`
- **Data directories**: `data/sd-cli-data/`, `data/sd-cli-data/instance-alice/`
- **CLI state**: Separate `cli_state.json` per instance

### File Structure

```
$runtime_dir/               # /tmp or $XDG_RUNTIME_DIR
├── spacedrive.sock         # Default instance socket
├── spacedrive.pid          # Default instance PID
├── spacedrive-alice.sock   # Alice instance socket
├── spacedrive-alice.pid    # Alice instance PID
├── spacedrive-bob.sock     # Bob instance socket
└── spacedrive-bob.pid      # Bob instance PID

data/sd-cli-data/                    # Default instance data
├── spacedrive.json
├── libraries/
└── cli_state.json

data/sd-cli-data/instance-alice/     # Alice instance data
├── spacedrive.json
├── libraries/
└── cli_state.json

data/sd-cli-data/instance-bob/       # Bob instance data
├── spacedrive.json
├── libraries/
└── cli_state.json
```

## Development Workflow

### Testing Pairing Protocol

```bash
# Start two instances for pairing test
spacedrive start --instance initiator --enable-networking --foreground &
spacedrive start --instance joiner --enable-networking --foreground &

# Initialize networking
spacedrive --instance initiator network init --password "dev123"
spacedrive --instance joiner network init --password "dev123"

# Test pairing
CODE=$(spacedrive --instance initiator network pair generate --auto-accept | grep "Pairing code:" | cut -d' ' -f3-)
spacedrive --instance joiner network pair join "$CODE"

# Verify connection
spacedrive --instance initiator network devices
spacedrive --instance joiner network devices
```

### Instance Cleanup

```bash
# Stop all instances
spacedrive instance list
spacedrive instance stop alice
spacedrive instance stop bob
spacedrive stop  # Default instance

# Clean up sockets (if needed)
rm /tmp/spacedrive*.sock /tmp/spacedrive*.pid
```

## Backwards Compatibility

The implementation maintains full backwards compatibility:
- All existing commands work unchanged with the default instance
- No breaking changes to CLI interface
- Default instance behavior is identical to single-instance mode

## Implementation Notes

- Instance names must be valid filenames (no special characters)
- Socket discovery happens automatically via filesystem scanning
- Daemon startup checks for instance conflicts
- Each instance runs independently with separate process trees
