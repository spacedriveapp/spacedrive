#!/bin/bash

# Script to run cross-device file copy integration tests

set -e

echo "🧪 Spacedrive Cross-Device File Copy Integration Tests"
echo "=================================================="
echo

# Check if we're in the right directory
if [[ ! -f "Cargo.toml" ]]; then
    echo "❌ Error: Please run this script from the core-new directory"
    exit 1
fi

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Cargo is not installed or not in PATH"
    echo "Please install Rust and Cargo: https://rustup.rs/"
    exit 1
fi

echo "🔧 Building project..."
cargo build --tests --bin test_core

echo
echo "🧪 Running cross-device file copy integration test..."
echo

# Function to run a test with proper error handling
run_test() {
    local test_name="$1"
    local description="$2"
    
    echo "▶️  $description"
    echo "   Test: $test_name"
    
    if RUST_LOG=info cargo test "$test_name" -- --nocapture; then
        echo "✅ $description - PASSED"
    else
        echo "❌ $description - FAILED"
        return 1
    fi
    echo
}

# Run the cross-device file copy test
echo "▶️  Cross-Device File Copy Test"
echo "   This test demonstrates:"
echo "   • Device pairing"
echo "   • File sharing API"
echo "   • Job system integration"
echo "   • Cross-device file transfer"
echo "   • File verification"
echo "   This test may take 1-2 minutes..."
echo

if timeout 150 cargo test test_cross_device_file_copy -- --nocapture; then
    echo "✅ Cross-Device File Copy Test - PASSED"
else
    echo "❌ Cross-Device File Copy Test - FAILED or TIMED OUT"
    echo "   Check the logs above for detailed error information"
    exit 1
fi

echo
echo "🎉 Cross-device file copy integration test completed successfully!"
echo
echo "📊 Test Summary:"
echo "   • Device pairing: ✅"
echo "   • Job system integration: ✅"  
echo "   • File transfer networking: ✅"
echo "   • File integrity verification: ✅"
echo
echo "💡 To run test manually:"
echo "   cargo test test_cross_device_file_copy"
echo "   RUST_LOG=debug cargo test test_cross_device_file_copy -- --nocapture"
echo
echo "🗂️  Test artifacts (if any) located in /tmp/received_files"