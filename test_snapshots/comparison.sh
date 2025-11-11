#!/bin/bash

echo "Checking if Event IDs match Database entry_uuids..."
echo ""

# Get a few event IDs
EVENT_IDS=$(jq -r '.[0][0:3] | .[] | .id' events_Content.json)

# Get DB UUIDs
DB_UUIDS=$(jq -r '.[] | .entry_uuid' db_entries_sample.json)

echo "Event IDs (first 3):"
echo "$EVENT_IDS" | head -3
echo ""

echo "DB entry_uuids (first 5):"
echo "$DB_UUIDS"
echo ""

# Check for any matches
echo "Checking for matches..."
for event_id in $EVENT_IDS; do
    if echo "$DB_UUIDS" | grep -q "$event_id"; then
        echo "MATCH FOUND: $event_id"
    fi
done

echo ""
echo "If no matches shown above, IDs don't match between events and DB"
