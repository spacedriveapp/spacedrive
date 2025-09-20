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

processed=0
modified=0

process_file() {
  local file="$1"
  FILE="$file" python3 <<'PYCODE'
import os, re, sys

path = os.environ['FILE']

# -------- Emoji building blocks --------
# Astral emoji/pictograph ranges
ASTRAL = (
    r'[\U0001F300-\U0001F5FF'   # Misc Symbols & Pictographs
    r'\U0001F600-\U0001F64F'    # Emoticons
    r'\U0001F680-\U0001F6FF'    # Transport & Map
    r'\U0001F700-\U0001F77F'    # Alchemical Symbols
    r'\U0001F780-\U0001F7FF'    # Geometric Shapes Extended
    r'\U0001F800-\U0001F8FF'    # Supplemental Arrows-C
    r'\U0001F900-\U0001F9FF'    # Supplemental Symbols & Pictographs
    r'\U0001FA00-\U0001FA6F'    # Symbols (chess, etc.)
    r'\U0001FA70-\U0001FAFF'    # Symbols & Pictographs Extended-A
    r'\U0001F1E6-\U0001F1FF]'   # Regional indicators (flags)
)

# BMP emoji-heavy ranges (cover ✅, ⏳, ⏸️, ℹ️, ✨, ☀️, arrows, enclosed nums, etc.)
BMP_EMOJI = (
    r'[\u2100-\u214F'  # Letterlike Symbols (ℹ️, ™️, ©️)
    r'\u2190-\u21FF'   # Arrows (→, ↔️, ⬆️)
    r'\u2300-\u23FF'   # Misc Technical (⏳, ⏸️, ⌚)
    r'\u2460-\u24FF'   # Enclosed Alphanumerics (①, Ⓜ️)
    r'\u2600-\u26FF'   # Misc Symbols (☀️, ☎️)
    r'\u2700-\u27BF]'  # Dingbats (✅, ❌, ✨)
)

# Modifiers and joiners
SKIN = r'[\U0001F3FB-\U0001F3FF]'  # Fitzpatrick skin tones
VS16 = r'\uFE0F'                   # emoji variation selector-16
ZWJ  = r'\u200D'                   # zero-width joiner
KC   = r'\u20E3'                   # keycap combining mark

# Keycap base (e.g., 1️⃣, *️⃣, #️⃣)
KEYCAP = r'[0-9#*]'

# Preserve exactly these text-style symbols followed by one space
# (Do NOT add colored emoji like ✅ here)
PRESERVE_LOOKAHEAD = r'(?![•✓→] )'

# Treat both BMP and astral emoji as BASE so they can take optional VS16 and be ZWJ-chained
BASE = rf'(?:{ASTRAL}|{BMP_EMOJI})'

# Generic emoji sequence:
#   BASE (optional SKIN) (optional VS16) (ZWJ BASE (optional SKIN) (optional VS16))*
EMOJI_SEQUENCE = rf'(?:{BASE}(?:{SKIN})?(?:{VS16})?(?:{ZWJ}{BASE}(?:{SKIN})?(?:{VS16})?)*)'

# Keycap sequence like 1️⃣, *️⃣, #️⃣
KEYCAP_SEQUENCE = rf'(?:{KEYCAP}{VS16}?{KC})'

# Final target: (emoji sequence OR keycap sequence) followed by exactly one space
emoji_regex = rf'{PRESERVE_LOOKAHEAD}(?:{EMOJI_SEQUENCE}|{KEYCAP_SEQUENCE}) '

pattern = re.compile(emoji_regex)

try:
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    cleaned = pattern.sub('', content)
    if cleaned != content:
        with open(path, 'w', encoding='utf-8') as f:
            f.write(cleaned)
        print('MODIFIED')
    else:
        print('UNCHANGED')
except Exception as e:
    print(f'ERROR: {e}')
    sys.exit(1)
PYCODE
}

echo -e "${BLUE}Processing files...${NC}"
echo

while IFS= read -r -d '' file; do
  ((processed++))
  printf "\r${BLUE}Progress: ${processed}/${total_files}${NC} - Processing: $(basename "$file")"

  result=$(process_file "$file")
  if [[ "$result" == "MODIFIED" ]]; then
    ((modified++))
    echo -e "\n${GREEN}✓ Modified: $file${NC}"
  elif [[ "$result" == ERROR* ]]; then
    echo -e "\n${RED}✗ Error processing: $file${NC}"
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
