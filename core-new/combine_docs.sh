#!/bin/bash

# Script to combine all documentation files into a single text file with headings
# Excludes the design directory

OUTPUT_FILE="combined_docs.txt"

# Clear the output file
> "$OUTPUT_FILE"

echo "Combining documentation files into $OUTPUT_FILE..."
echo "# Combined Documentation Files" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Find all .md files in docs directory, excluding design subdirectory
find docs -name "*.md" -type f ! -path "*/design/*" | sort | while read -r file; do
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

echo "Done! All documentation files have been combined into $OUTPUT_FILE"