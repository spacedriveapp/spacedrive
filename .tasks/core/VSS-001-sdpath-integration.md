---
id: VSS-001
title: "SdPath::Sidecar Variant Integration"
status: To Do
assignee: james
parent: CORE-008
priority: High
tags: [vss, addressing, sdpath, core]
whitepaper: "Section 4.1.5"
last_updated: 2025-11-01
related_tasks: [CORE-002, CORE-008]
---

## Description

Integrate sidecars as a first-class `SdPath` variant, enabling unified addressing and standard file operations. This elevates sidecars from specialized infrastructure to full VDFS citizens.

See `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md` Section "SdPath Integration" for complete specification.

## Implementation Files

- `core/src/domain/addressing.rs` - Add `Sidecar` variant to enum
- `core/src/domain/addressing/parser.rs` - Parse `sidecar://` URIs
- `core/src/domain/addressing/display.rs` - Display formatting
- `core/src/domain/addressing/resolver.rs` - Resolution logic
- `core/src/ops/sidecar/types.rs` - Ensure compatibility

## Tasks

### Add SdPath Variant
- [ ] Add `Sidecar { content_id, kind, variant, format }` to `SdPath` enum
- [ ] Implement `SdPath::sidecar()` helper method
- [ ] Add `is_sidecar()` predicate method
- [ ] Update `Clone`, `Debug`, `PartialEq` derives

### URI Parsing
- [ ] Implement `parse_sidecar()` for `sidecar://` scheme
- [ ] Parse format: `sidecar://{uuid}/{kind_dir}/{variant}.{ext}`
- [ ] Validate UUID, kind, variant, format
- [ ] Return helpful errors for malformed URIs

### Display Implementation
- [ ] Implement `Display` trait for sidecar paths
- [ ] Format as `sidecar://{uuid}/{kind}/{variant}.{ext}`
- [ ] Handle special characters in variants
- [ ] Add `to_uri()` method

### Resolution Integration
- [ ] Add `resolve_sidecar()` method to resolver
- [ ] Implement resolution modes (blocking, async, fetch-only)
- [ ] Integrate with existing `SidecarManager::compute_path()`
- [ ] Handle missing sidecars (enqueue generation)
- [ ] Handle remote sidecars (return device reference)

### Testing
- [ ] Unit tests for URI parsing (valid/invalid cases)
- [ ] Unit tests for display formatting
- [ ] Unit tests for resolution with various modes
- [ ] Integration test: parse → resolve → access
- [ ] Edge cases: special chars, missing sidecars, remote devices

## Acceptance Criteria

- [x] `SdPath::Sidecar` variant exists with all required fields
- [x] Can parse `sidecar://uuid/kind/variant.ext` strings
- [x] Display format matches specification
- [x] Helper methods work ergonomically
- [x] Resolution integrates with SidecarManager
- [x] Missing sidecars trigger generation in async mode
- [x] Remote sidecars return device references
- [x] All unit tests pass
- [x] Documentation updated in `docs/core/addressing.mdx`

## Example Usage

```rust
// Create sidecar path
let thumb = SdPath::sidecar(
    content_uuid,
    SidecarKind::Thumb,
    "grid@2x",
    SidecarFormat::Webp,
);

// Parse from URI
let parsed = SdPath::from_uri("sidecar://550e8400.../thumbs/grid@2x.webp")?;
assert_eq!(parsed, thumb);

// Display as URI
assert_eq!(thumb.display(), "sidecar://550e8400.../thumbs/grid@2x.webp");

// Resolve to physical path
let resolved = resolver.resolve(thumb).await?;
match resolved {
    ResolvedPath::Local(path) => read_file(path),
    ResolvedPath::Pending => wait_for_generation(),
    ResolvedPath::Remote(device_id, path) => fetch_from_device(device_id, path),
}
```

## Implementation Notes

### URI Format Specification

```
sidecar://{content_uuid}/{kind_directory}/{variant}.{extension}

Examples:
  sidecar://550e8400-e29b-41d4-a716-446655440000/thumbs/grid@2x.webp
  sidecar://550e8400-e29b-41d4-a716-446655440000/ocr/ocr.json
  sidecar://550e8400-e29b-41d4-a716-446655440000/embeddings/all-MiniLM-L6-v2.json

Glob patterns:
  sidecar://*/thumbs/grid@2x.webp              # All grid thumbnails
  sidecar://550e8400.../thumbs/*                # All thumbnails for one content
  sidecar://{uuid}/*                            # All sidecars for content
```

### Resolution Modes

```rust
pub enum SidecarResolveMode {
    /// Error if not available locally
    LocalOnly,

    /// Generate locally, block until ready
    GenerateBlocking,

    /// Generate locally, return pending
    GenerateAsync,

    /// Fetch from remote if available, else generate
    FetchOrGenerate,

    /// Fetch only, never generate
    FetchOnly,
}
```

### Integration with Existing Code

The `SidecarManager` service already exists with all the path computation logic. This task integrates it with the SdPath abstraction:

```rust
// Before: Direct SidecarManager usage
let path = sidecar_manager.compute_path(uuid, kind, variant, format)?;

// After: Through SdPath
let sidecar = SdPath::sidecar(uuid, kind, variant, format);
let resolved = resolver.resolve(sidecar)?;
```

Existing `SidecarManager` methods become implementation details behind the resolver.

## Timeline

Estimated: 3-4 days focused work

- Day 1: Add enum variant, helper methods, basic parsing
- Day 2: Display implementation, resolution logic
- Day 3: Testing, edge cases, error handling
- Day 4: Documentation, examples, polish
