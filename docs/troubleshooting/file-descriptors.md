# File Descriptor Issues

## Problem

You may encounter the error:
```
Error: Os { code: 24, kind: Uncategorized, message: "Too many open files" }
```

This happens when Spacedrive exceeds the system's limit on the number of file descriptors (files, sockets, etc.) that a process can have open simultaneously.

## Root Causes

1. **System Limits**: macOS and Linux have default limits on file descriptors per process
2. **Connection Leaks**: Previous versions had potential connection leaks in the RPC server
3. **Database Pools**: Multiple libraries with large connection pools can exhaust descriptors
4. **Concurrent Operations**: Many simultaneous operations can quickly reach limits

## Solutions

### Quick Fix (Temporary)

```bash
# Increase limit for current session
ulimit -n 65536

# Kill any stuck processes
pkill -f spacedrive

# Restart spacedrive
spacedrive start
```

### Permanent Fix

#### macOS

Add to your shell profile (`~/.zshrc` or `~/.bash_profile`):
```bash
ulimit -n 65536
```

Then reload:
```bash
source ~/.zshrc
```

#### Linux

Edit `/etc/security/limits.conf`:
```
* soft nofile 65536
* hard nofile 65536
```

Then restart your session.

### Diagnostic Tool

Run the diagnostic script:
```bash
./scripts/fix-file-descriptors.sh
```

This will:
- Check your current limits
- Show file usage by spacedrive processes
- Provide specific recommendations

## Improvements Made

### Connection Management
- Added connection limits (max 100 concurrent connections)
- Proper connection cleanup and counting
- Better error handling for EMFILE/ENFILE errors

### Database Optimization
- Reduced connection pool sizes from 10→5 max, 5→1 min
- More conservative resource usage per library

### Monitoring
- Connection statistics logging
- File descriptor limit warnings at startup
- Better error messages with diagnostic information

## Prevention

1. **Monitor Usage**: Check daemon logs for connection statistics
2. **Reasonable Limits**: Don't set limits too high (can cause system instability)
3. **Regular Restarts**: Restart the daemon periodically if running long-term
4. **Resource Management**: Avoid running too many concurrent operations

## System Requirements

- **Minimum**: 10,000 file descriptors
- **Recommended**: 65,536 file descriptors
- **Maximum**: 1,048,576 (system default on modern systems)

## Troubleshooting

If issues persist:

1. Check for stuck processes: `lsof | grep spacedrive`
2. Monitor connection count in daemon logs
3. Consider reducing concurrent operations
4. Check for other applications consuming file descriptors
5. Restart the system if limits seem corrupted

## Technical Details

The daemon now:
- Tracks active connections with atomic counters
- Rejects new connections when limit reached
- Properly cleans up connections on disconnect
- Uses smaller database connection pools
- Logs file descriptor limits at startup
