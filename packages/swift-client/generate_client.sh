#!/bin/bash

set -e

echo "🦀➡️🍎 Generating Spacedrive Swift Client"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCHEMA_FILE="../types.json"
GENERATED_TYPES_FILE="Sources/SpacedriveClient/types.swift"

# Check if schema file exists
if [ ! -f "$SCHEMA_FILE" ]; then
    echo -e "${RED}❌ Schema file not found: $SCHEMA_FILE${NC}"
    echo "Run 'cargo build' in the core directory first to generate the schema."
    exit 1
fi

# Check if quicktype is available
if ! command -v quicktype &> /dev/null; then
    echo -e "${RED}❌ quicktype not found${NC}"
    echo "Install quicktype with: npm install -g quicktype"
    exit 1
fi

# Step 1: Generate Event samples for proper enum generation
echo -e "${BLUE}Generating Event samples...${NC}"
cd ../.. && cargo run --bin generate_event_samples -p sd-core
cd packages/swift-client

# Step 2: Generate Swift Event enum from samples
echo -e "${BLUE}Generating Swift Event enum from samples...${NC}"
EVENT_SAMPLES_FILE="../event_samples.json"

if [ ! -f "$EVENT_SAMPLES_FILE" ]; then
    echo -e "${RED}❌ Event samples file not found: $EVENT_SAMPLES_FILE${NC}"
    exit 1
fi

quicktype "$EVENT_SAMPLES_FILE" \
    -o "$GENERATED_TYPES_FILE" \
    --lang swift \
    --top-level Event \
    --struct-or-class struct \
    --access-level public \
    --protocol none

if [ ! -f "$GENERATED_TYPES_FILE" ]; then
    echo -e "${RED}❌ Failed to generate types.swift${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Generated types.swift${NC}"

# Step 2: Build Swift package
echo -e "${BLUE}Building Swift package...${NC}"
swift build

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Swift build successful${NC}"
else
    echo -e "${RED}❌ Swift build failed${NC}"
    exit 1
fi

# Step 3: Run tests
echo -e "${BLUE}Running tests...${NC}"
swift test

echo -e "${GREEN}🎉 Swift client generation completed successfully!${NC}"
echo
echo -e "${YELLOW}📁 Generated files:${NC}"
echo "  - $GENERATED_TYPES_FILE (Generated Swift types)"
echo
echo -e "${YELLOW}🔄 To regenerate types after changing Rust structs:${NC}"
echo "  1. Run 'cargo build' in core directory"
echo "  2. Run './generate_client.sh' in this directory"
