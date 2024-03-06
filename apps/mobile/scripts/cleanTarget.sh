#!/bin/bash

# Check if the correct number of arguments is provided
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [android|ios]"
    exit 1
fi

# Set the target folder based on the first argument
TARGET_FOLDER="target"

# Check if the target folder exists
if [ ! -d "$TARGET_FOLDER" ]; then
    echo "Target folder '$TARGET_FOLDER' not found."
    exit 1
fi

# Set the keyword based on the first argument
KEYWORD=""
if [ "$1" == "android" ]; then
    KEYWORD="android"
elif [ "$1" == "ios" ]; then
    KEYWORD="ios"
else
    echo "Invalid argument. Please provide either 'android' or 'ios'."
    exit 1
fi

# Delete files based on the target folder and keyword
echo "Deleting files in '$TARGET_FOLDER' with keyword '$KEYWORD' in folder names..."

# Find and delete files in folders containing the specified keyword
find "$TARGET_FOLDER" -type d -name "*$KEYWORD*" -exec rm -r {} \;

# End of the script
echo "Files deleted successfully."
