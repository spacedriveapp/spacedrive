# Phase Snapshot Analysis - FINDINGS

## Confirmed: IDs DO MATCH! 

Event file with name "3AE4AB29-5EC8-4DF9-80F8-F72AE5C38FBF":
- Event ID: `0303b9df-3b11-49a6-beb6-0fdec577fefb`
- DB entry_uuid: `0303b9df-3b11-49a6-beb6-0fdec577fefb`

**Conclusion:** Events and database entries use the SAME UUID for File.id

## The Real Problem

If IDs match, why doesn't the normalized cache work?

Looking at the frontend logs from earlier:
```
Sample existing ID: "a72dfd9e-1908-4ddd-aa44-37458e8455ae"
Sample incoming ID: "0093bc71-a000-49f3-83f1-d1f72428665e"
WRAPPED: Updated 0 items
```

These IDs are different! This means:
1. The directory listing query returned files with certain UUIDs
2. The events arrived with DIFFERENT UUIDs (different files entirely)
3. No overlap = no matches = "Updated 0 items"

## Root Cause Theory

The events contain files from ALL OVER Desktop (100+ files per batch).
The directory listing only has ~30 files from the CURRENT directory.

Most batches have 0 overlap with the current directory, so:
- Updated 0 items (correct - those files aren't in this directory)
- But without proper filtering, the lag still happens from processing all events

## Solution

We need the `resourceFilter` to work, but it needs to handle Content paths.

The filter should check:
1. Does this file's `content_identity.uuid` match ANY file in my current directory query?
2. If yes → this file belongs here, update it
3. If no → ignore it (it's from a different directory)

This is what the current resourceFilter tries to do - match by content_id.
