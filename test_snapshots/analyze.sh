#!/bin/bash

echo "=============================================="
echo "TEMPORAL PHASE ANALYSIS"
echo "=============================================="
echo ""

# Check Discovery phase
if [ -f "phase_Discovery.json" ]; then
    echo "DISCOVERY PHASE:"
    DISCOVERY_COUNT=$(jq '.total_files' phase_Discovery.json)
    echo "   Total files: $DISCOVERY_COUNT"

    echo "   Sample file structure:"
    jq '.all_files[0] | {id, name, sd_path_type: (.sd_path | keys[0]), has_content_identity: (.content_identity != null)}' phase_Discovery.json
    echo ""
fi

# Check Content phase
if [ -f "phase_Content.json" ]; then
    echo "CONTENT PHASE:"
    CONTENT_COUNT=$(jq '.total_files' phase_Content.json)
    echo "   Total files: $CONTENT_COUNT"

    echo "   Sample file structure:"
    jq '.all_files[0] | {id, name, sd_path_type: (.sd_path | keys[0]), has_content_identity: (.content_identity != null)}' phase_Content.json

    echo ""
    echo "   Content identity breakdown:"
    WITH_CONTENT=$(jq '[.all_files[] | select(.content_identity != null)] | length' phase_Content.json)
    WITHOUT_CONTENT=$(jq '[.all_files[] | select(.content_identity == null)] | length' phase_Content.json)
    echo "   - With content_identity: $WITH_CONTENT"
    echo "   - Without content_identity: $WITHOUT_CONTENT"
    echo ""
fi

# Compare IDs between phases
echo "=============================================="
echo "ID CONSISTENCY CHECK"
echo "=============================================="
echo ""

if [ -f "phase_Discovery.json" ] && [ -f "phase_Content.json" ]; then
    DISC_ID=$(jq -r '.all_files[0].id' phase_Discovery.json)
    CONT_ID=$(jq -r '.all_files[0].id' phase_Content.json)

    echo "First file ID in Discovery: $DISC_ID"
    echo "First file ID in Content:   $CONT_ID"

    # Check if same file appears in both phases
    DISC_NAME=$(jq -r '.all_files[0].name' phase_Discovery.json)
    CONT_FILE=$(jq -r --arg name "$DISC_NAME" '.all_files[] | select(.name == $name) | .id' phase_Content.json | head -1)

    if [ -n "$CONT_FILE" ]; then
        echo ""
        echo "File '$DISC_NAME' found in both phases"
        echo "   Discovery ID: $DISC_ID"
        echo "   Content ID:   $CONT_FILE"
        if [ "$DISC_ID" == "$CONT_FILE" ]; then
            echo "   IDs MATCH"
        else
            echo "   IDs DIFFERENT"
        fi
    fi
fi

echo ""
echo "=============================================="
echo "DATABASE ENTRY COMPARISON"
echo "=============================================="
echo ""

# Check if event IDs exist in database
if [ -f "phase_Content.json" ] && [ -f "db_entries_all.json" ]; then
    EVENT_ID=$(jq -r '.all_files[0].id' phase_Content.json)
    EVENT_NAME=$(jq -r '.all_files[0].name' phase_Content.json)

    echo "Checking if event file exists in database:"
    echo "  Event file name: $EVENT_NAME"
    echo "  Event file ID: $EVENT_ID"
    echo ""

    DB_ENTRY=$(jq --arg id "$EVENT_ID" '.[] | select(.entry_uuid == $id)' db_entries_all.json)

    if [ -n "$DB_ENTRY" ]; then
        echo "  FOUND in database:"
        echo "$DB_ENTRY" | jq '{entry_id, entry_uuid, name}'
    else
        echo "  NOT FOUND in database"
        echo "  Searching by name instead..."
        DB_BY_NAME=$(jq --arg name "$EVENT_NAME" '.[] | select(.name == $name)' db_entries_all.json)
        if [ -n "$DB_BY_NAME" ]; then
            echo "  Found by name:"
            echo "$DB_BY_NAME" | jq '{entry_id, entry_uuid, name}'
        fi
    fi
fi

echo ""
echo "=============================================="
echo "FILES CREATED - Check these for full details:"
echo "=============================================="
ls -lh *.json *.md 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
echo ""
