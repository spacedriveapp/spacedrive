#!/bin/bash

echo "=============================================="
echo "DETAILED STRUCTURE COMPARISON"
echo "=============================================="
echo ""

# Get first file from each phase
echo "DISCOVERY PHASE - First File:"
echo "================================"
jq '.all_files[0]' phase_Discovery.json > /tmp/discovery_file.json
cat /tmp/discovery_file.json | jq '.'
echo ""

echo "CONTENT PHASE - First File:"
echo "================================"
jq '.all_files[0]' phase_Content.json > /tmp/content_file.json
cat /tmp/content_file.json | jq '.'
echo ""

echo "=============================================="
echo "FIELD-BY-FIELD COMPARISON"
echo "=============================================="
echo ""

# Compare each field
echo "Field: id"
DISC_ID=$(jq -r '.id' /tmp/discovery_file.json)
CONT_ID=$(jq -r '.id' /tmp/content_file.json)
echo "  Discovery: $DISC_ID"
echo "  Content:   $CONT_ID"
echo ""

echo "Field: sd_path"
echo "  Discovery:"
jq '.sd_path' /tmp/discovery_file.json
echo "  Content:"
jq '.sd_path' /tmp/content_file.json
echo ""

echo "Field: content_identity"
echo "  Discovery:"
jq '.content_identity | if . == null then "NULL" else ("Present - uuid: " + .uuid) end' /tmp/discovery_file.json
echo "  Content:"
jq '.content_identity | if . == null then "NULL" else ("Present - uuid: " + .uuid) end' /tmp/content_file.json
echo ""

echo "Field: sidecars"
DISC_SIDECARS=$(jq '.sidecars | length' /tmp/discovery_file.json)
CONT_SIDECARS=$(jq '.sidecars | length' /tmp/content_file.json)
echo "  Discovery: $DISC_SIDECARS sidecars"
echo "  Content:   $CONT_SIDECARS sidecars"
echo ""

echo "=============================================="
echo "SCHEMA DIFFERENCES"
echo "=============================================="
echo ""

echo "All top-level fields in Discovery file:"
jq 'keys | sort' /tmp/discovery_file.json
echo ""

echo "All top-level fields in Content file:"
jq 'keys | sort' /tmp/content_file.json
echo ""

echo "Checking if field sets are identical..."
DISC_FIELDS=$(jq -r 'keys | sort | join(",")' /tmp/discovery_file.json)
CONT_FIELDS=$(jq -r 'keys | sort | join(",")' /tmp/content_file.json)

if [ "$DISC_FIELDS" == "$CONT_FIELDS" ]; then
    echo "Both phases have IDENTICAL field sets"
else
    echo "Different fields between phases!"
    echo ""
    echo "Fields only in Discovery:"
    comm -23 <(jq -r 'keys[]' /tmp/discovery_file.json | sort) <(jq -r 'keys[]' /tmp/content_file.json | sort)
    echo ""
    echo "Fields only in Content:"
    comm -13 <(jq -r 'keys[]' /tmp/discovery_file.json | sort) <(jq -r 'keys[]' /tmp/content_file.json | sort)
fi

echo ""
echo "=============================================="
echo "CONTENT_IDENTITY DEEP COMPARISON"
echo "=============================================="
echo ""

echo "Discovery content_identity fields:"
jq '.content_identity | if . != null then keys | sort else "null" end' /tmp/discovery_file.json
echo ""

echo "Content content_identity fields:"
jq '.content_identity | if . != null then keys | sort else "null" end' /tmp/content_file.json
echo ""

echo "Are content_identity structures identical?"
DISC_CI_FIELDS=$(jq -r '.content_identity | if . != null then (keys | sort | join(",")) else "null" end' /tmp/discovery_file.json)
CONT_CI_FIELDS=$(jq -r '.content_identity | if . != null then (keys | sort | join(",")) else "null" end' /tmp/content_file.json)

if [ "$DISC_CI_FIELDS" == "$CONT_CI_FIELDS" ]; then
    echo "YES - identical structure"
else
    echo "NO - different structure"
    echo "Discovery: $DISC_CI_FIELDS"
    echo "Content:   $CONT_CI_FIELDS"
fi

echo ""
echo "=============================================="
echo "SAMPLE: Complete File Structure Comparison"
echo "=============================================="
echo ""

# Show side-by-side comparison of a complete file structure
echo "Discovery file saved to: /tmp/discovery_file.json"
echo "Content file saved to: /tmp/content_file.json"
echo ""
echo "Use: diff /tmp/discovery_file.json /tmp/content_file.json"
echo "Or:  code --diff /tmp/discovery_file.json /tmp/content_file.json"

