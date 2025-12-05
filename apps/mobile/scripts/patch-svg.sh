#!/bin/bash
# Patch react-native-svg for Yoga compatibility with React Native 0.79

set -e

echo "Patching react-native-svg for Yoga compatibility..."

# Find all instances of the file in bun cache
find "$(dirname "$0")/../../node_modules" -path "*react-native-svg*" -name "RNSVGLayoutableShadowNode.cpp" 2>/dev/null | while read file; do
  if [ -f "$file" ]; then
    echo "Patching: $file"
    # Replace yoga::StyleLength with yoga::Style::SizeLength for setDimension calls
    sed -i '' 's/yoga::StyleLength::percent/yoga::Style::SizeLength::percent/g' "$file"
    sed -i '' 's/yoga::StyleLength::points/yoga::Style::SizeLength::points/g' "$file"
  fi
done

# Also check root node_modules
find "$(dirname "$0")/../../../node_modules" -path "*react-native-svg*" -name "RNSVGLayoutableShadowNode.cpp" 2>/dev/null | while read file; do
  if [ -f "$file" ]; then
    echo "Patching: $file"
    sed -i '' 's/yoga::StyleLength::percent/yoga::Style::SizeLength::percent/g' "$file"
    sed -i '' 's/yoga::StyleLength::points/yoga::Style::SizeLength::points/g' "$file"
  fi
done

echo "Patch complete!"
