#!/bin/bash
# Spacedrive CLI Demo Script

echo "🚀 Spacedrive CLI Demo"
echo "====================="
echo ""

# Build the CLI
echo "📦 Building Spacedrive CLI..."
cargo build --release --bin spacedrive

# Set CLI alias for convenience
SD="./target/release/spacedrive"

echo "✅ CLI built successfully!"
echo ""

# Show help
echo "📖 Showing CLI help:"
echo "==================="
$SD --help
echo ""

# Create a library
echo "📚 Creating a new library:"
echo "========================="
$SD library create "Demo Library"
echo ""

# Show library list
echo "📋 Listing libraries:"
echo "===================="
$SD library list
echo ""

# Show current library
echo "📍 Current library:"
echo "=================="
$SD library current
echo ""

# Add a location
echo "📁 Adding Desktop as a location:"
echo "==============================="
$SD location add ~/Desktop --name "Desktop" --mode content
echo ""

# List locations
echo "📋 Listing locations:"
echo "===================="
$SD location list
echo ""

# Show system status
echo "🖥️  System status:"
echo "================="
$SD status
echo ""

# List jobs
echo "💼 Listing jobs:"
echo "==============="
$SD job list
echo ""

echo "✨ Demo complete!"
echo ""
echo "🎯 Try these commands:"
echo "  - Monitor jobs in real-time: $SD job monitor"
echo "  - Index a specific folder: $SD index ~/Documents --watch"
echo "  - Switch libraries: $SD library switch <name-or-id>"
echo "  - Get location info: $SD location info <id-or-path>"
echo ""