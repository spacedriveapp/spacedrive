# FINAL ANALYSIS - Normalized Cache Issue

## Test Results Summary

**Test:** Indexed Desktop (235 files total) with Content mode

### Phase Event Breakdown

| Phase | Files Emitted | Content Identity |
|-------|---------------|------------------|
| Discovery | 100 | All have content_identity |
| Content | 135 | All have content_identity |

### Critical Finding: ZERO OVERLAP

**Discovery files vs Content files:** 0 files appear in both phases

This means:
- Discovery phase emits events for files 1-100
- Content phase emits events for files 101-235 (different subset)
- **They emit for completely different files**

## Why This Breaks Normalized Cache

### Scenario: User viewing /Downloads with 30 files

**Step 1: Initial load**
- Query returns 30 files with IDs: `[A, B, C, ..., Z]`
- Cache populated

**Step 2: Discovery phase events arrive**
- Batch #1: 100 files with IDs `[file001, file002, ..., file100]`
- Check: Do any of these IDs match `[A, B, C, ..., Z]`?
- Answer: **Maybe 5-10 files** if they're from /Downloads
- Result: Update 5 files, ignore 95

**Step 3: Content phase events arrive**  
- Batch #1: 100 files with IDs `[file101, file102, ..., file200]`
- These are DIFFERENT files from Discovery
- Check: Do any match `[A, B, C, ..., Z]`?
- Answer: **Maybe 10 files** if from /Downloads
- Result: Update 10 files

### The Problem

Files get emitted in DIFFERENT batches across phases:
- File "document.pdf" emitted in Discovery phase
- File "photo.jpg" emitted in Content phase
- BUT they might be in the SAME directory

When Content phase events arrive, they DON'T include "document.pdf" again.
So if Discovery event had incomplete data, it never gets fixed!

## Data Confirms: Events Have Complete Data

From snapshots:
- Discovery: 100/100 files have content_identity 
- Content: 135/135 files have content_identity 

**Events are NOT missing content_identity!**

## The Real Bug

The logs show "Updated 0 items" which means:
1. Event batch arrives with 100 files
2. Current directory has 30 files
3. **ZERO IDs match** between event and directory
4. Nothing gets updated

But you said content_identity disappears... 

Let me check if maybe the resourceFilter is REJECTING all files, causing React Query to have an empty cache.

## Next Step

Check the resourceFilter logic - it might be rejecting ALL files, leaving an empty array.
