#!/bin/bash

# Pick the first file from Discovery phase
DISCOVERY_FILE=$(jq '.all_files[0]' phase_Discovery.json)
FILE_ID=$(echo "$DISCOVERY_FILE" | jq -r '.id')
FILE_NAME=$(echo "$DISCOVERY_FILE" | jq -r '.name')

echo "=============================================="
echo "TRACKING FILE: $FILE_NAME"
echo "ID: $FILE_ID"
echo "=============================================="
echo ""

echo "DISCOVERY PHASE:"
echo "$DISCOVERY_FILE" | jq '{
  id,
  name,
  sd_path,
  content_identity_present: (.content_identity != null),
  content_identity_uuid: .content_identity.uuid,
  sidecars_count: (.sidecars | length)
}'
echo ""

echo "CONTENT PHASE (same file):"
CONTENT_FILE=$(jq --arg id "$FILE_ID" '.all_files[] | select(.id == $id)' phase_Content.json)

if [ -n "$CONTENT_FILE" ]; then
    echo "$CONTENT_FILE" | jq '{
      id,
      name,
      sd_path,
      content_identity_present: (.content_identity != null),
      content_identity_uuid: .content_identity.uuid,
      sidecars_count: (.sidecars | length)
    }'
else
    echo "   File not found in Content phase events"
fi
echo ""

echo "=============================================="
echo "COMPARISON:"
echo "=============================================="

DISC_HAS_CI=$(echo "$DISCOVERY_FILE" | jq '.content_identity != null')
CONT_HAS_CI=$(echo "$CONTENT_FILE" | jq '.content_identity != null')

echo "Discovery has content_identity: $DISC_HAS_CI"
echo "Content has content_identity:   $CONT_HAS_CI"
echo ""

# Check full objects
echo "FULL DISCOVERY FILE:"
echo "$DISCOVERY_FILE" | jq '.'
echo ""

if [ -n "$CONTENT_FILE" ]; then
    echo "FULL CONTENT FILE:"
    echo "$CONTENT_FILE" | jq '.'
fi
