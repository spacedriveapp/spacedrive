#!/bin/bash

echo "Checking file overlap between phases..."
echo ""

# Get all IDs from each phase
DISCOVERY_IDS=$(jq -r '.all_files[].id' phase_Discovery.json | sort)
CONTENT_IDS=$(jq -r '.all_files[].id' phase_Content.json | sort)

DISCOVERY_COUNT=$(echo "$DISCOVERY_IDS" | wc -l | tr -d ' ')
CONTENT_COUNT=$(echo "$CONTENT_IDS" | wc -l | tr -d ' ')

echo "Discovery phase: $DISCOVERY_COUNT files"
echo "Content phase: $CONTENT_COUNT files"
echo ""

# Find overlapping IDs
OVERLAP=$(comm -12 <(echo "$DISCOVERY_IDS") <(echo "$CONTENT_IDS") | wc -l | tr -d ' ')

echo "Files appearing in BOTH phases: $OVERLAP"
echo ""

if [ "$OVERLAP" -gt 0 ]; then
    echo "Some files appear in both phases"
    echo ""
    echo "Sample overlapping file:"
    OVERLAP_ID=$(comm -12 <(echo "$DISCOVERY_IDS") <(echo "$CONTENT_IDS") | head -1)
    echo "ID: $OVERLAP_ID"
    
    echo ""
    echo "In Discovery:"
    jq --arg id "$OVERLAP_ID" '.all_files[] | select(.id == $id) | {name, has_content_identity: (.content_identity != null)}' phase_Discovery.json
    
    echo ""
    echo "In Content:"
    jq --arg id "$OVERLAP_ID" '.all_files[] | select(.id == $id) | {name, has_content_identity: (.content_identity != null)}' phase_Content.json
else
    echo "NO files appear in both phases"
    echo "   Discovery emits one set of files"
    echo "   Content emits a completely different set"
    echo "   This is expected if they're different batches/directories"
fi
