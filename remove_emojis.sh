#!/bin/bash

# Script to remove 3D/colorful emojis followed by a space from Rust files
# To counter Claude's obsession with emojis
# Preserves simple symbols like •, ✓, →
# Usage: ./remove_emojis.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Spacedrive Emoji Removal Script${NC}"
echo "Removing 3D/colorful emojis with trailing spaces from Rust files..."
echo "Preserving simple symbols like •, ✓, →"
echo

# Count files to process
total_files=$(find . -name "*.rs" -type f | wc -l | tr -d ' ')
echo -e "${BLUE}Found ${total_files} Rust files to process${NC}"
echo

# No backup needed - using git for version control

# Counter for processed files
processed=0
modified=0

# Function to check if file contains emojis with spaces
contains_emoji_space() {
    local file="$1"
    # Use Python to detect 3D/colorful emojis followed by spaces
    python3 -c "
import re
import sys

# Read file content
try:
    with open('$file', 'r', encoding='utf-8') as f:
        content = f.read()

    # Only target 3D/colorful emojis followed by exactly one space
    emoji_pattern = r'[\U0001F600-\U0001F64F\U0001F300-\U0001F5FF\U0001F680-\U0001F6FF\U0001F780-\U0001F7FF\U0001F1E0-\U0001F1FF\U0001F900-\U0001F9FF\U0001FA00-\U0001FA6F\U0001FA70-\U0001FAFF] '

    if re.search(emoji_pattern, content):
        sys.exit(0)  # Found emoji with space
    else:
        sys.exit(1)  # No emoji with space found
except Exception as e:
    sys.exit(1)  # Error reading file
"
    return $?
}

# Function to remove emojis with spaces from a file
remove_emojis() {
    local file="$1"

    # Use Python to remove emojis followed by spaces
    python3 -c "
import re
import sys

# Read file content
try:
    with open('$file', 'r', encoding='utf-8') as f:
        content = f.read()

    # Only target 3D/colorful emojis followed by exactly one space
    emoji_pattern = r'[\U0001F600-\U0001F64F\U0001F300-\U0001F5FF\U0001F680-\U0001F6FF\U0001F780-\U0001F7FF\U0001F1E0-\U0001F1FF\U0001F900-\U0001F9FF\U0001FA00-\U0001FA6F\U0001FA70-\U0001FAFF] '

    # Remove emojis followed by space
    cleaned_content = re.sub(emoji_pattern, '', content)

    # Write back to file
    with open('$file', 'w', encoding='utf-8') as f:
        f.write(cleaned_content)

    # Check if file was actually modified
    if content != cleaned_content:
        print('MODIFIED')
    else:
        print('UNCHANGED')

except Exception as e:
    print(f'ERROR: {e}')
    sys.exit(1)
"
}

# Process all Rust files
echo -e "${BLUE}Processing files...${NC}"
echo

while IFS= read -r -d '' file; do
    ((processed++))

    # Show progress
    printf "\r${BLUE}Progress: ${processed}/${total_files}${NC} - Processing: $(basename "$file")"

    # Check if file contains emojis with spaces
    if contains_emoji_space "$file"; then
        result=$(remove_emojis "$file")
        if [[ "$result" == "MODIFIED" ]]; then
            ((modified++))
            echo -e "\n${GREEN}✓ Modified: $file${NC}"
        elif [[ "$result" == "ERROR"* ]]; then
            echo -e "\n${RED}✗ Error processing: $file${NC}"
        fi
    fi

done < <(find . -name "*.rs" -type f -print0)

echo -e "\n\n${GREEN}✅ Processing complete!${NC}"
echo -e "${BLUE}Summary:${NC}"
echo -e "  Total files processed: ${processed}"
echo -e "  Files modified: ${modified}"
echo

if [[ $modified -gt 0 ]]; then
    echo -e "${GREEN}${modified} files were modified.${NC}"
    echo -e "${YELLOW}Use 'git diff' to see changes or 'git checkout .' to revert${NC}"
else
    echo -e "${GREEN}No files needed modification.${NC}"
fi

echo -e "\n${GREEN}Done!${NC}"
