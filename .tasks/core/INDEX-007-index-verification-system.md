---
id: INDEX-007
title: Index Verification System
status: Done
assignee: jamiepine
parent: INDEX-000
priority: Medium
tags: [indexing, verification, integrity, diagnostics]
whitepaper: Section 4.3.8
last_updated: 2025-12-16
---

## Description

Implement the index integrity verification system that detects discrepancies between filesystem state and database records. The system runs a fresh ephemeral scan and compares metadata against the persistent index to identify missing, stale, or mismatched entries.

## Architecture

### IndexVerifyAction

The verification action runs as a library action (not a job) for fast diagnostics:

```rust
pub struct IndexVerifyAction {
    path: PathBuf,
}

pub struct IndexVerifyOutput {
    pub report: IntegrityReport,
}

pub struct IntegrityReport {
    pub missing_from_index: Vec<MissingFile>,
    pub stale_in_index: Vec<StaleFile>,
    pub metadata_mismatches: Vec<MetadataMismatch>,
    pub summary: Summary,
}

pub struct MissingFile {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

pub struct StaleFile {
    pub path: PathBuf,
    pub entry_id: i32,
    pub last_indexed: SystemTime,
}

pub struct MetadataMismatch {
    pub path: PathBuf,
    pub entry_id: i32,
    pub issue: MismatchKind,
}

pub enum MismatchKind {
    SizeMismatch { db: u64, fs: u64 },
    ModifiedTimeMismatch { db: SystemTime, fs: SystemTime },
    InodeMismatch { db: u64, fs: u64 },
}

pub struct Summary {
    pub total_files_in_db: usize,
    pub total_files_on_fs: usize,
    pub missing_count: usize,
    pub stale_count: usize,
    pub mismatch_count: usize,
}
```

### Verification Process

1. **Run Ephemeral Scan**: Index the path in memory (Phase 1 only)
2. **Load Database Entries**: Query existing entries for the same path
3. **Compare**: For each filesystem entry, check against database:
   - **MissingFromIndex**: File exists on disk but not in database
   - **StaleInIndex**: Entry in database but file missing from filesystem
   - **SizeMismatch**: Size differs between database and filesystem
   - **ModifiedTimeMismatch**: Mtime differs (with 1-second tolerance)
   - **InodeMismatch**: Inode changed (file replacement or corruption)
4. **Generate Report**: Detailed diagnostics with per-file breakdowns

### Comparison Logic

```rust
async fn compare_entries(
    ephemeral_index: &EphemeralIndex,
    db_entries: &HashMap<PathBuf, EntryRecord>,
) -> IntegrityReport {
    let mut report = IntegrityReport::default();

    // Check each filesystem file against database
    for (path, ephemeral_node) in ephemeral_index.iter() {
        if let Some(db_entry) = db_entries.get(path) {
            // File exists in both, check metadata
            if ephemeral_node.size != db_entry.size {
                report.metadata_mismatches.push(MetadataMismatch {
                    path: path.clone(),
                    entry_id: db_entry.id,
                    issue: MismatchKind::SizeMismatch {
                        db: db_entry.size,
                        fs: ephemeral_node.size,
                    },
                });
            }

            // Allow 1-second tolerance for mtime (filesystem precision varies)
            let time_diff = ephemeral_node.modified.abs_diff(db_entry.modified);
            if time_diff > Duration::from_secs(1) {
                report.metadata_mismatches.push(MetadataMismatch {
                    path: path.clone(),
                    entry_id: db_entry.id,
                    issue: MismatchKind::ModifiedTimeMismatch {
                        db: db_entry.modified,
                        fs: ephemeral_node.modified,
                    },
                });
            }

            if ephemeral_node.inode != db_entry.inode {
                report.metadata_mismatches.push(MetadataMismatch {
                    path: path.clone(),
                    entry_id: db_entry.id,
                    issue: MismatchKind::InodeMismatch {
                        db: db_entry.inode,
                        fs: ephemeral_node.inode,
                    },
                });
            }
        } else {
            // File on disk but not in database
            report.missing_from_index.push(MissingFile {
                path: path.clone(),
                size: ephemeral_node.size,
                modified: ephemeral_node.modified,
            });
        }
    }

    // Check for stale database entries (not on disk)
    for (path, db_entry) in db_entries.iter() {
        if !ephemeral_index.contains(path) {
            report.stale_in_index.push(StaleFile {
                path: path.clone(),
                entry_id: db_entry.id,
                last_indexed: db_entry.indexed_at,
            });
        }
    }

    report.summary = Summary {
        total_files_in_db: db_entries.len(),
        total_files_on_fs: ephemeral_index.len(),
        missing_count: report.missing_from_index.len(),
        stale_count: report.stale_in_index.len(),
        mismatch_count: report.metadata_mismatches.len(),
    };

    report
}
```

## Implementation Files

### Verification Action
- `core/src/ops/indexing/verify/action.rs` - IndexVerifyAction implementation
- `core/src/ops/indexing/verify/input.rs` - IndexVerifyInput
- `core/src/ops/indexing/verify/output.rs` - IndexVerifyOutput and IntegrityReport
- `core/src/ops/indexing/verify/mod.rs` - Module exports

### Integration
- `core/src/ops/indexing/action.rs` - Action registration
- `core/src/ops/mod.rs` - Action exports

## Acceptance Criteria

- [x] IndexVerifyAction runs fresh ephemeral scan of path
- [x] Action loads existing database entries for comparison
- [x] MissingFromIndex detects files on disk but not in database
- [x] StaleInIndex detects entries in database but missing from filesystem
- [x] SizeMismatch detects size differences
- [x] ModifiedTimeMismatch detects mtime differences (1-second tolerance)
- [x] InodeMismatch detects inode changes
- [x] Report includes summary statistics
- [x] Report provides per-file diagnostics
- [x] Verification runs as library action (not job)
- [x] Fast execution (ephemeral scan only, no database writes)
- [x] CLI command exposes verification

## Use Cases

### Post-Offline Detection

After app has been offline, verify index integrity:

```bash
spacedrive verify ~/Documents
```

**Expected Issues**:
- Files created externally → MissingFromIndex
- Files deleted externally → StaleInIndex
- Files modified externally → SizeMismatch or ModifiedTimeMismatch

### Debugging Watcher Issues

If real-time updates seem broken, verify state:

```bash
spacedrive verify /media/usb
```

**Expected Issues**:
- Missed create events → MissingFromIndex
- Missed delete events → StaleInIndex
- Missed modify events → MetadataMismatch

### Pre-Migration Validation

Before migrating to new library version, verify current state:

```bash
spacedrive verify --all-locations
```

Ensures clean state before schema migrations.

## CLI Integration

```bash
# Verify specific path
spacedrive verify ~/Documents

# Verify all locations
spacedrive verify --all-locations

# Verify with detailed output
spacedrive verify ~/Pictures --verbose

# Output JSON for scripting
spacedrive verify ~/Videos --json > report.json
```

## Output Format

### Console Output

```
Index Verification Report
=========================
Path: /Users/jamie/Documents
Scanned: 15,234 files
Database: 15,180 entries

Issues Found:
-------------
Missing from index: 54 files
Stale in index: 12 entries
Metadata mismatches: 8 files

Details:
--------
Missing from index:
  /Users/jamie/Documents/new_file.txt (created 2025-10-14)
  /Users/jamie/Documents/another.pdf (created 2025-10-14)
  ...

Stale in index:
  /Users/jamie/Documents/deleted.txt (last seen 2025-10-01)
  /Users/jamie/Documents/old.doc (last seen 2025-09-15)
  ...

Metadata mismatches:
  /Users/jamie/Documents/modified.txt
    - Size: DB=1024, FS=2048
  /Users/jamie/Documents/touched.pdf
    - Modified: DB=2025-10-01 12:00:00, FS=2025-10-14 14:30:00
  ...

Recommendation: Run reindex to fix issues
```

### JSON Output

```json
{
  "path": "/Users/jamie/Documents",
  "summary": {
    "total_files_in_db": 15180,
    "total_files_on_fs": 15234,
    "missing_count": 54,
    "stale_count": 12,
    "mismatch_count": 8
  },
  "missing_from_index": [
    {
      "path": "/Users/jamie/Documents/new_file.txt",
      "size": 2048,
      "modified": "2025-10-14T10:30:00Z"
    }
  ],
  "stale_in_index": [
    {
      "path": "/Users/jamie/Documents/deleted.txt",
      "entry_id": 12345,
      "last_indexed": "2025-10-01T08:00:00Z"
    }
  ],
  "metadata_mismatches": [
    {
      "path": "/Users/jamie/Documents/modified.txt",
      "entry_id": 12346,
      "issue": {
        "kind": "SizeMismatch",
        "db": 1024,
        "fs": 2048
      }
    }
  ]
}
```

## Performance Characteristics

| Location Size | Verification Time | Notes |
|--------------|------------------|-------|
| 1K files | <1 second | Ephemeral scan + comparison |
| 10K files | 2-5 seconds | Depends on disk speed |
| 100K files | 20-50 seconds | Mostly filesystem traversal |
| 1M files | 3-5 minutes | Batched comparison |

**Bottleneck**: Filesystem traversal (Phase 1 discovery), not comparison.

## Testing

### Manual Testing

```bash
# Create test location with known state
mkdir -p ~/test-verify
cd ~/test-verify
touch file1.txt file2.txt file3.txt

# Index it
spacedrive index location ~/test-verify --mode shallow

# Make external changes
touch external_new.txt
rm file2.txt
echo "modified" >> file3.txt

# Verify (should detect issues)
spacedrive verify ~/test-verify

# Expected output:
# - Missing: external_new.txt
# - Stale: file2.txt
# - Mismatch: file3.txt (size/mtime changed)
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_verify_missing_from_index` - Detect new files
- `test_verify_stale_in_index` - Detect deleted files
- `test_verify_size_mismatch` - Detect size changes
- `test_verify_mtime_mismatch` - Detect mtime changes
- `test_verify_inode_mismatch` - Detect file replacement
- `test_verify_clean_index` - No issues when in sync

## Future Enhancements

- **Auto-Fix Mode**: `--fix` flag to automatically reindex mismatched files
- **Incremental Verification**: Only verify changed directories (via mtime)
- **Scheduled Verification**: Periodic background integrity checks
- **Notification**: Alert user when issues exceed threshold
- **Metrics**: Track verification results over time

## Related Tasks

- INDEX-001 - Hybrid Architecture (uses ephemeral scan for verification)
- INDEX-004 - Change Detection (verification detects missed changes)
- INDEX-002 - Five-Phase Pipeline (verification uses Phase 1 only)
