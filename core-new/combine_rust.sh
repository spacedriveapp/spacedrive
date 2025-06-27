#!/bin/bash

# Script to combine all Rust files into a single text file with headings
# Respects .gitignore patterns

OUTPUT_FILE="combined_rust_files.txt"

# Function to check if a file should be ignored based on .gitignore patterns
is_ignored() {
    local file_path=$1
    
    # Use git check-ignore to respect .gitignore rules
    if git check-ignore "$file_path" >/dev/null 2>&1; then
        return 0  # File is ignored
    else
        return 1  # File is not ignored
    fi
}

# Clear the output file
> "$OUTPUT_FILE"

echo "Combining Rust files into $OUTPUT_FILE..."
echo "# Combined Rust Files" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Find all .rs files and process them
find . -name "*.rs" -type f | while read -r file; do
    # Skip if the file is ignored by git
    if is_ignored "$file"; then
        continue
    fi
    
    # Remove leading ./ from path for cleaner output
    clean_path=${file#./}
    
    echo "Processing: $clean_path"
    
    # Add heading for the file
    echo "## $clean_path" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo '```rust' >> "$OUTPUT_FILE"
    
    # Add the file content
    cat "$file" >> "$OUTPUT_FILE"
    
    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo "Done! All Rust files have been combined into $OUTPUT_FILE"