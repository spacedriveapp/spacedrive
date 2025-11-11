# ROOT CAUSE IDENTIFIED - Missing Icons During Indexing

## The Bug

During indexing, video files (and other media) lose their proper icons and fall back to the generic "Document" icon. The icons are restored when thumbnails are generated, but for files without thumbnails (like videos with thumbnail generation disabled), the icons remain wrong.

## What We Thought It Was

- content_identity disappearing (normalized cache bug)
- React not re-rendering
- TanStack Query not updating
- Event system not working

## What It Actually Is

**The `content_kind` field is hardcoded to "unknown" in event data**

## The Evidence

### From Browser Logs

All 4 videos render with complete data:
```
Screen Recording 2025-11-09 at 7.18.50 PM:
  has_content_identity: true
  content_identity_uuid: "06358a76-0974-50c9-a939-70b70a910a91"
  sidecars_count: 0
```

### From TanStack Query Devtools

Final state shows all videos have `content_identity` populated with all fields present.

### From Event Snapshots (test_snapshots/)

```json
{
  "content_identity": {
    "uuid": "...",
    "content_hash": "...",
    "kind": "unknown"  // THIS IS THE PROBLEM
  }
}
```

## How The Icon System Works

**Thumb.tsx line 66-67:**
```typescript
const kindCapitalized = file.content_identity?.kind
  ? file.content_identity.kind.charAt(0).toUpperCase() + file.content_identity.kind.slice(1)
  : "Document";

const icon = getIcon(kindCapitalized, true, file.extension, ...);
```

**When kind = "unknown":**
- Capitalizes to "Unknown"
- `getIcon("Unknown", ...)` returns generic document icon
- Videos, images, etc. all show document icon ❌

**When kind = "video":**
- Capitalizes to "Video"
- `getIcon("Video", ...)` returns video icon
- Correct icon shows ✅

## The Root Cause Code

**File:** `core/src/domain/file.rs:354`

```rust
file.content_identity = Some(ContentIdentity {
    uuid: ci_uuid,
    content_hash: ci.content_hash.clone(),
    integrity_hash: ci.integrity_hash.clone(),
    mime_type_id: ci.mime_type_id,
    kind: ContentKind::Unknown, // TODO: Load from content_kinds table
    total_size: ci.total_size,
    entry_count: ci.entry_count,
    first_seen_at: ci.first_seen_at,
    last_verified_at: ci.last_verified_at,
    text_content: ci.text_content.clone(),
});
```

**The TODO comment says it all:** "Load from content_kinds table"

## Why Thumbnails "Fix" It

When thumbnail events arrive, they trigger a re-render and the thumbnail image loads, hiding the icon entirely. So you don't notice the wrong icon anymore. But the underlying data is still wrong - `kind` is still "unknown".

## The Fix

In `File::from_entry_uuids()`, we need to:

1. Load the `content_kind` for each `content_identity`
2. Join with the `content_kinds` table
3. Map the `kind_id` to `ContentKind` enum
4. Set the correct kind instead of `ContentKind::Unknown`

**The database already has this data** - we just need to query it:

```rust
// Load content kinds
let content_kinds = content_kind::Entity::find()
    .filter(content_kind::Column::Id.is_in(
        content_identities.iter().map(|ci| ci.kind_id)
    ))
    .all(db)
    .await?;

let kind_by_id: HashMap<i32, ContentKind> = content_kinds
    .into_iter()
    .map(|ck| (ck.id, ContentKind::from_id(ck.id)))
    .collect();

// Then when building ContentIdentity:
kind: kind_by_id.get(&ci.kind_id).copied().unwrap_or(ContentKind::Unknown),
```

## Why The Normalized Cache Actually Works Perfectly

The entire investigation proved the normalized cache is working correctly:

Events are emitted with complete data (all 100% have content_identity)
IDs match between events and queries (entry.uuid)
Deep merge preserves existing data correctly
Filter matches files by content_id successfully
TanStack Query updates atomically
React re-renders when cache changes
Components receive updated props

The ONLY bug: `content_kind` is hardcoded to "unknown" in event data, causing wrong icons.

## Test Results Summary

- **phase_Discovery.json**: 100 files, all with `kind: "unknown"`
- **phase_Content.json**: 135 files, all with `kind: "unknown"`
- **db_entries_all.json**: Database has 235 entries with proper content_id foreign keys

The database HAS the correct content_kind data. The directory listing query probably loads it correctly (TODO: verify). But `File::from_entry_uuids()` doesn't load it, so events have incomplete kind information.

## Next Steps

1. Add content_kind join to `File::from_entry_uuids()`
2. Map kind_id to ContentKind enum
3. Test that icons appear correctly during indexing
4. Remove all the debug logging we added

That's it. One small database join fixes everything.
