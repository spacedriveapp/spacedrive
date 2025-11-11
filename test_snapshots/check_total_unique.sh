#!/bin/bash

echo "Checking TOTAL UNIQUE files across all phases..."
echo ""

# Get all unique IDs from Discovery
DISCOVERY_IDS=$(jq -r '.all_files[].id' phase_Discovery.json | sort -u)

# Get all unique IDs from Content  
CONTENT_IDS=$(jq -r '.all_files[].id' phase_Content.json | sort -u)

# Combine and count unique
ALL_IDS=$(echo -e "$DISCOVERY_IDS\n$CONTENT_IDS" | sort -u)
TOTAL_UNIQUE=$(echo "$ALL_IDS" | wc -l | tr -d ' ')

DISCOVERY_COUNT=$(echo "$DISCOVERY_IDS" | wc -l | tr -d ' ')
CONTENT_COUNT=$(echo "$CONTENT_IDS" | wc -l | tr -d ' ')

echo "Discovery phase: $DISCOVERY_COUNT unique file IDs"
echo "Content phase:   $CONTENT_COUNT unique file IDs"
echo "Total unique:    $TOTAL_UNIQUE file IDs"
echo ""
echo "Database entries: 235"
echo ""

if [ "$TOTAL_UNIQUE" -eq 235 ]; then
    echo "Total unique matches DB count!"
    echo "   Discovery + Content = exactly 235 files"
    echo "   They are DIFFERENT files (no duplicates)"
elif [ "$TOTAL_UNIQUE" -lt 235 ]; then
    echo "Total unique ($TOTAL_UNIQUE) < 235"
    echo "   Some files appear in BOTH phases"
    OVERLAP=$((DISCOVERY_COUNT + CONTENT_COUNT - TOTAL_UNIQUE))
    echo "   Overlap: $OVERLAP files"
else
    echo "️  Total unique ($TOTAL_UNIQUE) > 235"
    echo "   This shouldn't happen - more files in events than DB?"
fi
