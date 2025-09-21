#!/bin/bash

set -e

echo "🦀➡️📜 Generating Spacedrive TypeScript Client"
echo "============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCHEMA_FILE="../types.json"
GENERATED_FILE="src/types.ts"

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

# Generate TypeScript types from unified schema
echo -e "${BLUE}Generating TypeScript types from unified schema...${NC}"

quicktype "$SCHEMA_FILE" \
    -o "$GENERATED_FILE" \
    --lang typescript

if [ ! -f "$GENERATED_FILE" ]; then
    echo -e "${RED}❌ Failed to generate types.ts${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Generated types.ts${NC}"

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo -e "${BLUE}Installing dependencies...${NC}"
    npm install
fi

# Build TypeScript
echo -e "${BLUE}Building TypeScript...${NC}"
npm run build

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ TypeScript build successful${NC}"
else
    echo -e "${RED}❌ TypeScript build failed${NC}"
    exit 1
fi

# Run tests
echo -e "${BLUE}Running tests...${NC}"
npm test

echo -e "${GREEN}🎉 TypeScript client generation completed successfully!${NC}"
echo
echo -e "${YELLOW}📁 Generated files:${NC}"
echo "  - $GENERATED_FILE (Generated TypeScript types)"
echo "  - dist/ (Compiled JavaScript)"
echo
echo -e "${YELLOW}🔄 To regenerate types after changing Rust structs:${NC}"
echo "  1. Run 'cargo build' in core directory"
echo "  2. Run './generate_client.sh' in this directory"
