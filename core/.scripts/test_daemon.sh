#!/bin/bash

# Start daemon in background
./target/release/spacedrive daemon start --foreground 2>&1 | tee daemon_test.log &
DAEMON_PID=$!

# Wait for daemon to initialize
sleep 3

# Try to create a library
echo "Creating library..."
./target/release/spacedrive library create "TestLib" 2>&1

# Check if daemon is still running
if kill -0 $DAEMON_PID 2>/dev/null; then
    echo "Daemon is still running"
else
    echo "Daemon crashed!"
fi

# Stop daemon
kill $DAEMON_PID 2>/dev/null

# Show any errors from the log
echo "Last 20 lines of daemon log:"
tail -20 daemon_test.log