#!/bin/bash

# Script to combine all task files into a single text file with headings
# Excludes the design directory

OUTPUT_FILE="combined_tasks.txt"

# Clear the output file
> "$OUTPUT_FILE"

echo "Combining task files into $OUTPUT_FILE..."
echo "# Combined task Files" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Find all .md files in .tasks directory
find .tasks -name "*.md" -type f | sort | while read -r file; do
    echo "Processing: $file"

    # Add heading for the file
    echo "## $file" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo '```markdown' >> "$OUTPUT_FILE"

    # Add the file content
    cat "$file" >> "$OUTPUT_FILE"

    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo "Done! All task files have been combined into $OUTPUT_FILE"
