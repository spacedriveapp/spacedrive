#!/bin/bash

# A generic script to combine files from a directory into a single file.

# Function to show usage
usage() {
    echo  "Usage: $0 <command> [options]"
    echo ""
    echo  "Commands:"
    echo  "  docs [--with-design]    Combine documentation files (.md) from 'docs/'."
    echo  "                           --with-design: Include the 'docs/design' directory."
    echo  "  rust [path]             Combine Rust files (.rs) from a given path (default: '.')."
    echo  "                          Respects .gitignore."
    echo  "  cli                     Combine Rust files (.rs) from 'apps/cli'."
    echo  "                          Respects .gitignore."
    echo  "  tasks                   Combine task files (.md) from '.tasks/'."

    echo ""
    echo  "Options:"
    echo  "-h, --help              Show this help message."
}

# Function to check if a file should be ignored based on .gitignore
is_ignored() {
    git check-ignore -q "$1"
}

# Main combine function
combine_files() {
    local search_path=$1
    local file_pattern=$2
    local output_file=$3
    local title=$4
    local lang_tag=$5
    local respect_gitignore=$6
    shift 6
    local exclude_patterns=("$@")

    echo "Combining files from '$search_path' into '$output_file'..."

    # Clear the output file
    > "$output_file"

    echo "# $title" >> "$output_file"
    echo "" >> "$output_file"

    # Build find args and exclude patterns
    local find_args=("$search_path" -name "$file_pattern" -type f)
    for pattern in "${exclude_patterns[@]}"; do
        find_args+=("!" "-path" "$pattern")
    done

    # Run find with exec to safely process files one-by-one
    find "${find_args[@]}" | sort | while read -r file; do
        if [ "$respect_gitignore" = "true" ] && git check-ignore -q "$file"; then
            continue
        fi

        echo "Processing: $file"
        echo "## ${file}" >> "$output_file"
        echo "" >> "$output_file"
        echo '```'$lang_tag >> "$output_file"

        if [ -r "$file" ]; then
            cat "$file" >> "$output_file"
        else
            echo "[Could not read file]" >> "$output_file"
        fi

        echo '```' >> "$output_file"
        echo "" >> "$output_file"
    done

    echo "Done! All files have been combined into $output_file"
}

# Main script logic
if [ "$#" -eq 0 ]; then
    usage
    exit 1
fi

COMMAND=$1
shift

case $COMMAND in
    docs)
        include_design=false
        if [ "$1" = "--with-design" ]; then
            include_design=true
        fi

        exclude_patterns=()
        if [ "$include_design" = "false" ]; then
            exclude_patterns+=("../docs/design/*")
        fi

        combine_files "../docs" "*.md" "combined_docs.txt" "Combined Documentation Files" "markdown" "false" "${exclude_patterns[@]}"
         ;;
    rust)
        root_path=${1:-.}
        combine_files "$root_path" "*.rs" "combined_rust_files.txt" "Combined Rust Files" "rust" "true"
         ;;
    cli)
        combine_files "./apps/cli" "*.rs" "combined_cli_rust_files.txt" "Combined CLI Rust Files" "rust" "true"
         ;;
    tasks)
        combine_files "../.tasks" "*.md" "combined_tasks.txt" "Combined Task Files" "markdown" "false"
         ;;
    -h|--help)
        usage
         ;;
    *)
        echo "Error: Unknown command '$COMMAND'"
        usage
        exit 1
         ;;
esac
