# Spacedrive CLI Redesign Plan

## Overview

Redesign the CLI structure to be more intuitive, consistent, and user-friendly while maintaining power-user capabilities. All top-level commands (except start/stop) will support interactive wizards when called without arguments, making the CLI approachable for new users while keeping direct command paths for scripting.

## Core Design Principles

1. **Interactive by Default**: Commands without args enter interactive mode
2. **Hybrid SdPath Support**: Accept both traditional paths and SdPath URIs (`local://`, `s3://`, `content://`)
3. **Consistent Patterns**: Every resource type has predictable subcommands (list, create, remove, etc.)
4. **Smart Context Awareness**: Commands adapt based on context (e.g., browse uses index when available)
5. **Scriptable**: All interactive flows have direct command equivalents with `--format json` support

## Command Structure

### Daemon Lifecycle (No Wizards)

```bash
sd start [--foreground]           # Start daemon
sd stop [--reset]                 # Stop daemon (optional data reset)
```

### Configuration

```bash
sd config                         # Interactive: show current config, prompt to edit
sd config get <key>              # Get specific config value
sd config set <key> <value>      # Set config value
```

### Library Management

```bash
sd library                        # Show current library status (name, locations, stats, devices)
sd library create                 # Interactive: name, path, settings
sd library switch                 # Interactive: select from list
sd library list                   # List all libraries
sd library delete                 # Interactive: select + confirm
```

### Location Management (Managed Directories)

```bash
sd location                       # Interactive: list → add/remove/rescan
sd location add                   # Interactive wizard (already implemented)
sd location remove                # Interactive: select from list
sd location rescan [id]          # Interactive: select location if no ID
sd location list                  # List all locations
```

### Universal Browsing (Location-Aware)

```bash
sd browse [path|uri]              # Smart browsing with interactive TUI
                                  # - Uses location index if path is managed
                                  # - Falls back to ephemeral index if outside locations
                                  # - No path = interactive root picker
```

**Behavior:**

- Inside managed location: instant (uses existing index)
- Outside locations: ephemeral index (temporary, not persisted)
- Supports SdPath URIs for remote browsing

### File Operations (Hybrid SdPath)

```bash
sd ls [path|uri]                  # List files (simple output)
sd cp <src> <dst>                 # Copy (supports URIs + --device/--cloud flags)
sd mv <src> <dst>                 # Move
sd rm <path|uri>                  # Delete (with confirmation)
sd info <path|uri>                # Show file metadata
```

**SdPath Examples:**

```bash
# Traditional paths
sd cp /Users/me/file.txt /backup/

# SdPath URIs
sd cp local://macbook/Users/me/file.txt s3://my-bucket/backup/
sd info content://550e8400-e29b-41d4-a716-446655440000
```

### Global Search

```bash
sd search                         # Interactive: query builder with filters
sd search <query>                 # Direct search
sd search --tag <tag>            # Filter by tag
sd search --type <type>          # Filter by file type
sd search --content <text>       # Content search
sd search --size <range>         # Size filter
sd search --date <range>         # Date filter
```

### Organization - Tags

```bash
sd tag                            # Interactive: select file → add tags
sd tag create                     # Interactive: name, color, namespace
sd tag apply <target> <tags>     # Direct apply tags
sd tag remove <target> <tags>    # Remove tags
sd tag list                       # List all tags
sd tag search <query>            # Search tag names (different from sd search)
```

### Organization - Collections

```bash
sd collection                     # Interactive: list → create/add/remove
sd collection create              # Interactive: name, description
sd collection add <id>           # Interactive: select files to add
sd collection remove <id>        # Interactive: select files to remove
sd collection list                # List all collections
```

### Network - Pairing

```bash
sd pair                           # Interactive: initiate or join
sd pair initiate                  # Generate pairing code
sd pair join [code]              # Interactive: enter code if not provided
```

### Network - Devices

Note: These are paired devices, not devices registered in a library, for clarity we should show which libraries these devices are participating in by quering the devices table for all libraries!

```bash
sd devices                        # Interactive: list → revoke/manage
sd devices list                   # List paired devices
sd devices remove <id>           # Remove/revoke device
```

### Network - File Sharing

This doesn't exist yet so we can implement as a stub

```bash
sd share                          # Interactive: select device → select file
sd share <device> <file>         # Direct share via Spacedrop
```

### Cloud Storage

```bash
sd cloud                          # Interactive wizard (already implemented)
sd cloud add                      # Interactive: service type → credentials
sd cloud remove                   # Interactive: select volume
sd cloud list                     # List cloud volumes
```

### Volumes

```bash
sd volume                         # Interactive: list → manage
sd volume list                    # List all volumes (local + cloud)
```

### Sync Conduits (WIP Feature)

```bash
sd sync                           # Interactive: conduit management
sd sync status                    # Show sync state
sd sync create                    # Interactive: create sync conduit
```

### Jobs & Monitoring

```bash
sd job                            # Interactive: list → monitor/pause/cancel
sd job list                       # List all jobs
sd job monitor [id]              # Monitor jobs with TUI (all or specific)
sd job pause <id>                # Pause job
sd job resume <id>               # Resume job
sd job cancel <id>               # Cancel job
```

### Logs

```bash
sd logs                           # Interactive: show or follow
sd logs show [--tail N]          # Show recent logs
sd logs follow                    # Follow logs in real-time
```

## Removed/Merged Commands

### Removed

- `sd index` → Functionality absorbed into `sd location` and `sd browse`
- `sd status` → Replaced by `sd library` (shows current state)
- `sd network` → Split into `sd pair`, `sd devices`, `sd share`
- `sd restart` → Can be achieved with `sd stop && sd start`
- `sd update` → Can be system-level or `sd daemon update` if needed

### Merged/Reorganized

- `sd location browse` → `sd browse` (root level, location-aware)
- `sd index quick-scan` → `sd browse` (ephemeral mode automatic)
- `sd index start` → `sd location add` (with mode flags)
- `sd index verify` → `sd location rescan --verify`
- `sd network pair` → `sd pair`
- `sd network devices` → `sd devices`
- `sd network spacedrop` → `sd share`

## Implementation Plan

### Phase 1: Command Restructure

**Goal**: Reorganize command structure and file layout

**Tasks:**

1. Create new domain modules:
   - `apps/cli/src/domains/browse/` (new)
   - `apps/cli/src/domains/pair/` (split from network)
   - `apps/cli/src/domains/share/` (split from network)
   - `apps/cli/src/domains/collection/` (new)

2. Remove obsolete modules:
   - `apps/cli/src/domains/index/` (merge into location + browse)

3. Update `apps/cli/src/main.rs`:
   - Restructure `Commands` enum to match new hierarchy
   - Remove merged commands
   - Update command routing

4. Update existing domain modules to match new patterns

### Phase 2: Smart Browse Implementation

**Goal**: Create location-aware browsing command

**Tasks:**

1. Implement browse command with dual-mode indexing:
   - Check if path is within managed location
   - Use location index if available (fast)
   - Fall back to ephemeral index if outside (slower)

2. Add interactive TUI for navigation:
   - Tree view or grid view
   - Keyboard navigation
   - Preview panel
   - Reference existing location wizard UX

3. Support SdPath URIs for remote browsing:
   - `sd browse local://device/path`
   - `sd browse s3://bucket/prefix`

### Phase 3: Enhanced Search

**Goal**: Restore and improve global search

**Tasks:**

1. Restore `domains/search/` with enhanced functionality
2. Implement filter flags:
   - `--tag`, `--type`, `--content`, `--size`, `--date`
3. Create interactive query builder
4. Multiple output formats: table, json, paths-only

### Phase 4: Interactive Wizards

**Goal**: Add wizards to all commands that should have them

**Commands requiring wizards:**

- `sd config` - show/edit flow
- `sd browse` - TUI navigator
- `sd search` - query builder
- `sd tag` - tagging workflow
- `sd collection` - collection management
- `sd pair` - pairing flow
- `sd devices` - device management
- `sd share` - file sharing picker
- `sd volume` - volume management
- `sd job` - job list → actions
- `sd logs` - show/follow picker

**Implementation approach:**

- Use `dialoguer` crate for prompts
- Show contextual info before prompts
- Maintain direct command paths for scripting
- Pattern: detect when called with no subcommand/args

### Phase 5: Hybrid SdPath Support

**Goal**: Support both traditional paths and SdPath URIs

**Tasks:**

1. Create URI parser that accepts both formats
2. Update file operations (ls, cp, mv, rm, info, browse):
   - Parse traditional paths: `/Users/me/file.txt`
   - Parse SdPath URIs: `local://device/path`, `s3://bucket/key`, `content://uuid`
   - Support shortcut flags: `--device`, `--cloud`

3. Add resolution logic:
   - Convert traditional paths to SdPath internally
   - Resolve URIs to actual storage locations
   - Handle cross-device operations

4. Error handling:
   - Clear messages for malformed URIs
   - Suggestions for common mistakes

### Phase 6: Documentation & Polish

**Goal**: Comprehensive documentation and UX refinement

**Tasks:**

1. Update help text for all commands
2. Add examples in `--help` output
3. Update documentation files in `docs/cli/`
4. Create migration guide from old commands to new
5. Add shell completions (bash, zsh, fish)
6. Test all interactive flows
7. Ensure `--format json` works consistently

## Success Criteria

- All commands follow consistent patterns
- Interactive mode works for all designated commands
- Direct command paths work for scripting
- SdPath URIs work across file operations
- Browse intelligently uses location index when available
- Search provides powerful filtering
- Documentation is comprehensive
- No regression in existing functionality
- Shell completions work

## Migration Notes

**For users upgrading from current CLI:**

Breaking changes:

- `sd index` removed → use `sd location add` or `sd browse`
- `sd network pair` → `sd pair`
- `sd network spacedrop` → `sd share`
- `sd status` → `sd library`

Non-breaking:

- All location commands remain the same
- Library commands remain the same
- Job commands remain the same
- Logs commands remain the same

## Design Rationale

### Why `browse` is separate from `ls`

- `browse` is an interactive navigator with TUI
- `ls` is a simple list command (like traditional Unix ls)
- Different use cases: exploration vs scripting

### Why `search` is global while `tag search` exists

- `sd search` searches file content, names, metadata across entire library
- `sd tag search` searches for tag names themselves
- Different domains: files vs tags

### Why split `network` into `pair`, `devices`, `share`

- Each has distinct mental models and workflows
- Pairing is an infrequent setup task
- Devices is for ongoing management
- Share is a frequent operation that should be quick

### Why remove `sd status`

- `sd library` provides library-level status (the most common query)
- System-level status can be `sd daemon status` if needed
- Reduces command clutter

### Why keep `sd config` separate

- Global configuration spans libraries
- Different scope than library-specific settings
- Common pattern in other CLIs (git config, npm config)
