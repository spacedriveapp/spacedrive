# File Data Model - Visual Comparison

## Old Model (v1) - Current Spacedrive

```
┌─────────────┐       ┌────────────┐       ┌───────────┐
│  Location   │───────│  FilePath  │───────│  Object   │
└─────────────┘  1:n  └────────────┘  n:1  └───────────┘
                            │                      │
                            │                      │
                      cas_id (nullable)      ┌─────┴─────┐
                            │                │           │
                            │             Tags      Labels
                      (content hash)   (via junction tables)

Problems:
- No Object = No tags (non-indexed files can't be tagged)
- cas_id required to create Object
- Tight coupling between content identity and user metadata
```

## New Model (v2) - Proposed

```
┌─────────────┐       ┌─────────────┐       ┌──────────────┐
│  Location   │───────│    Entry    │───────│UserMetadata  │
└─────────────┘  1:n  └─────────────┘  1:1  └──────────────┘
                            │                       │
                      sd_path: SdPath          Tags, Labels,
                   (device + local path)       Notes, etc.
                            │                  (ALWAYS exists)
                            │
                            │ content_id
                            │ (nullable)
                            ▼
                    ┌────────────────┐
                    │ContentIdentity │
                    └────────────────┘
                            │
                      cas_id, mime_type,
                      media_data, etc.
                    (ONLY if indexed)

Benefits:
- Every Entry has UserMetadata (can tag any file)
- ContentIdentity is optional (for deduplication)
- Clean separation of concerns
```

## Key Relationships

### 1. Entry ↔ UserMetadata (1:1)
```
Entry                          UserMetadata
├─ id: uuid                    ├─ id: uuid
├─ sd_path: SdPath            ├─ tags: []
├─ name: "photo.jpg"          ├─ labels: []
├─ metadata_id ───────────────├─ notes: "Vacation 2024"
└─ content_id: uuid?          └─ favorite: true
```

### 2. Entry ↔ ContentIdentity (n:1)
```
Entry (MacBook)                ContentIdentity
├─ id: uuid-1                  ├─ id: uuid
├─ sd_path: mac:/photo.jpg     ├─ cas_id: "v2:a1b2c3..."
├─ content_id ─────────────────├─ kind: Image
                               ├─ mime_type: "image/jpeg"
Entry (iPhone)                 ├─ entry_count: 2
├─ id: uuid-2                  └─ total_size: 10MB
├─ sd_path: iphone:/DCIM/...
└─ content_id ─────────────────┘

(Same content on different devices)
```

## Process Flow Comparison

### Old Flow: Must Index to Tag
```
1. Discover File
   └─> Create FilePath (no tags possible yet)

2. Index Content (required for tags)
   ├─> Read file content
   ├─> Generate cas_id
   └─> Create/Link Object

3. Now Can Tag
   └─> Add tags to Object
```

### New Flow: Tag Immediately
```
1. Discover File
   ├─> Create Entry
   └─> Create UserMetadata (can tag immediately!)

2. Index Content (optional, async)
   ├─> Read file content
   ├─> Generate cas_id
   └─> Create/Link ContentIdentity

3. Tags Unaffected by Content Changes
```

## SdPath Serialization Examples

### Option 1: Separate Columns
```sql
CREATE TABLE entry (
    id UUID PRIMARY KEY,
    device_id UUID NOT NULL,
    path TEXT NOT NULL,
    library_id UUID,
    -- ... other fields
    UNIQUE(device_id, path, library_id)
);
```

### Option 2: JSON Column
```sql
CREATE TABLE entry (
    id UUID PRIMARY KEY,
    sd_path JSONB NOT NULL,
    -- Example: {"device_id": "abc", "path": "/home/user/file.txt", "library_id": null}
    -- ... other fields
);

-- Can still index and query efficiently:
CREATE INDEX idx_entry_device ON entry((sd_path->>'device_id'));
CREATE INDEX idx_entry_path ON entry((sd_path->>'path'));
```

### Option 3: Custom Format String
```sql
CREATE TABLE entry (
    id UUID PRIMARY KEY,
    sd_path TEXT NOT NULL,
    -- Format: "device_id://path" or "library_id/device_id://path"
    -- Example: "a1b2c3d4://home/user/file.txt"
    -- ... other fields
);

-- With computed columns for efficiency:
ALTER TABLE entry ADD COLUMN device_id UUID 
    GENERATED ALWAYS AS (split_part(sd_path, '://', 1)::UUID) STORED;
ALTER TABLE entry ADD COLUMN local_path TEXT 
    GENERATED ALWAYS AS (split_part(sd_path, '://', 2)) STORED;
```

## Query Examples

### Find all copies of a file (by content)
```sql
-- New model: Find all entries with same content
SELECT e.*, um.*
FROM entry e
JOIN user_metadata um ON e.metadata_id = um.id
WHERE e.content_id = ?;

-- Old model: Find all file_paths with same object
SELECT fp.*, o.*
FROM file_path fp
JOIN object o ON fp.object_id = o.id
WHERE o.id = ?;
```

### Tag a non-indexed file
```sql
-- New model: Just works!
UPDATE user_metadata 
SET tags = array_append(tags, 'Important')
WHERE id = (SELECT metadata_id FROM entry WHERE sd_path = ?);

-- Old model: Impossible! No object exists
-- Would need to force content indexing first
```

### Find files modified after tagging
```sql
-- New model: Tags persist through content changes
SELECT e.*, um.*
FROM entry e
JOIN user_metadata um ON e.metadata_id = um.id
LEFT JOIN content_identity ci ON e.content_id = ci.id
WHERE 'Important' = ANY(um.tags)
  AND e.modified_at > um.updated_at;

-- Old model: Content changes could break object association
-- Complex logic needed to track this
```