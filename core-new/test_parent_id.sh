#!/bin/bash

# Test script to verify parent_id is being set correctly

echo "Testing parent_id implementation..."

# Build the project
echo "Building project..."
cargo build --example desktop_indexing_demo

# Run the demo
echo "Running indexing demo..."
cargo run --example desktop_indexing_demo

# Query the database to check parent_id values
echo "Checking database for parent_id values..."
echo "SELECT id, name, relative_path, parent_id, kind FROM entries ORDER BY id LIMIT 20;" | sqlite3 ~/.local/share/spacedrive/libraries/*/spacedrive.db 2>/dev/null || echo "Database not found or no entries yet"

echo "Done!"