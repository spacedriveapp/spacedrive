#!/bin/bash
# Quick test to see the initial logs from the companion app

cd /Users/jamespine/Projects/spacedrive/apps/spacedrive-companion

echo "🧪 Testing companion app logs..."
echo "🧪 Building and starting app..."

# Build first
swift build

# Run and capture output, then kill after 8 seconds
(swift run &
APP_PID=$!
sleep 8
kill $APP_PID 2>/dev/null
wait $APP_PID 2>/dev/null) 2>&1 | head -50

echo "🧪 Test completed"
