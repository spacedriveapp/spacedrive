#!/bin/bash

# Spacedrive File Descriptor Fix Script
# This script helps diagnose and fix "Too many open files" errors

echo "🔍 Spacedrive File Descriptor Diagnostic Tool"
echo "=============================================="

# Check current limits
echo "📊 Current System Limits:"
echo "Soft limit: $(ulimit -Sn)"
echo "Hard limit: $(ulimit -Hn)"

# Check if limits are too low
SOFT_LIMIT=$(ulimit -Sn)
if [ "$SOFT_LIMIT" -lt 10000 ]; then
    echo "⚠️  WARNING: Soft limit ($SOFT_LIMIT) is quite low!"
    echo "   This may cause 'Too many open files' errors."
else
    echo "✅ Soft limit looks reasonable"
fi

# Check current spacedrive processes
echo ""
echo "🔍 Current Spacedrive File Usage:"
SPACEDRIVE_FILES=$(lsof 2>/dev/null | grep -i spacedrive | wc -l)
echo "Open files by spacedrive processes: $SPACEDRIVE_FILES"

if [ "$SPACEDRIVE_FILES" -gt 500 ]; then
    echo "⚠️  WARNING: High file usage detected!"
    echo "   This may indicate a file descriptor leak."
else
    echo "✅ File usage looks normal"
fi

# Check for running daemon processes
echo ""
echo "🔍 Running Daemon Processes:"
DAEMON_PIDS=$(pgrep -f "spacedrive.*daemon" 2>/dev/null)
if [ -n "$DAEMON_PIDS" ]; then
    echo "Found daemon processes: $DAEMON_PIDS"
    for pid in $DAEMON_PIDS; do
        FILES_FOR_PID=$(lsof -p "$pid" 2>/dev/null | wc -l)
        echo "  PID $pid: $FILES_FOR_PID open files"
    done
else
    echo "No daemon processes found"
fi

echo ""
echo "🛠️  Solutions:"
echo "=============="

# Solution 1: Temporary fix
echo "1. Temporary fix (current session only):"
echo "   ulimit -n 65536"
echo "   Then restart spacedrive"

# Solution 2: Permanent fix for macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo ""
    echo "2. Permanent fix for macOS:"
    echo "   Create/edit ~/.zshrc or ~/.bash_profile:"
    echo "   echo 'ulimit -n 65536' >> ~/.zshrc"
    echo "   source ~/.zshrc"
fi

# Solution 3: System-wide fix
echo ""
echo "3. System-wide fix (requires admin):"
echo "   Edit /etc/launchd.conf (macOS) or /etc/security/limits.conf (Linux)"
echo "   Add: limit maxfiles 65536 200000"

# Solution 4: Kill existing processes
echo ""
echo "4. If you have stuck processes:"
echo "   pkill -f spacedrive"
echo "   Then restart spacedrive"

echo ""
echo "💡 Additional Tips:"
echo "==================="
echo "- The daemon now has connection limits to prevent resource exhaustion"
echo "- Database connection pools have been optimized to use fewer file descriptors"
echo "- Check daemon logs for connection statistics"
echo "- Consider reducing the number of concurrent operations if issues persist"

echo ""
echo "🚀 Quick Fix Command:"
echo "===================="
echo "ulimit -n 65536 && pkill -f spacedrive && sleep 2 && spacedrive start"
