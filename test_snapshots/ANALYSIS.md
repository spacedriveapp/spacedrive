# Phase Snapshot Analysis

## Event File Structure (from events_Content.json)

```json
{
  "id": "01f866cf-ab4a-474b-800d-6e10f1445aae",
  "name": "TUPLE_VARIANTS_IMPLEMENTED",
  "sd_path": {
    "Content": {
      "content_id": "3e457574-0b4a-51e8-80ae-ad05f78193d7"
    }
  },
  "content_identity": "3e457574-0b4a-51e8-80ae-ad05f78193d7"
}
```

## Database Entry Structure (from db_entries_sample.json)

```json
{
  "entry_id": 8,
  "entry_uuid": "26977785-98eb-4de7-a77b-dc63daef44b3",
  "name": "Screen Recording 2025-11-10 at 2.42.22 AM",
  "content_id_fk": 1
}
```

## Key Findings

**Event Files:**
- `id` = entry UUID (e.g., `"01f866cf-ab4a-474b-800d-6e10f1445aae"`)
- `sd_path` = Content path with `content_id`
- `content_identity.uuid` matches `sd_path.Content.content_id`

**Database Entries:**
- `entry_uuid` = UUID stored in entry table
- `content_id_fk` = Foreign key to content_identities table (integer)

**Directory Query (expected):**
- Should use `entry_uuid` as File `id`
- Should use Physical `sd_path` like `/Users/jamespine/Desktop/file.txt`

## The Problem

Events and queries should have matching IDs (both use `entry_uuid`), but:
1. Event `sd_path` = Content type
2. Query `sd_path` = Physical type

This means path-based filtering can't work, but ID-based matching SHOULD work if both use the same UUID.

Check the actual directory listing query to confirm it uses `entry_uuid` as the File `id`.
