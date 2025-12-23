---
id: VSS-004
title: "Cross-Device Sidecar Sync"
status: To Do
assignee: jamiepine
parent: CORE-008
priority: Medium
tags: [vss, sync, networking, p2p]
whitepaper: "Section 4.1.5"
last_updated: 2025-11-01
related_tasks: [CORE-008, LSYNC-000, NET-000]
dependencies: [VSS-001, VSS-002]
---

## Description

Implement cross-device sidecar discovery and transfer, enabling devices to share and reuse generated sidecars instead of regenerating them locally. This is critical for "generate once, use everywhere" efficiency.

**Discovery Model:** Devices periodically gossip availability digests over the network. When looking for a sidecar, devices query connected peers directly (not via database sync). The `sidecar_availability` table tracks only what the current device has locally.

See `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md` Section "Cross-Device Sync" for complete specification.

## Implementation Files

- `core/src/service/sync/sidecar_sync.rs` - New file for availability sync
- `core/src/service/network/protocol/sidecar_transfer.rs` - Transfer protocol
- `core/src/service/sidecar_manager.rs` - Add remote availability methods

## Tasks

### Availability Discovery Protocol
- [ ] Implement `AvailabilityDigest` structure
- [ ] Create digest from local sidecars (what THIS device has)
- [ ] Implement digest gossip via network protocol
- [ ] Cache peer availability in memory (short-lived, rebuilable)
- [ ] Add periodic gossip scheduler (every 5 minutes)
- [ ] Implement network query: "Do you have sidecar X?"

### Sidecar Transfer Protocol
- [ ] Implement `SidecarTransferJob`
- [ ] Reuse existing P2P file transfer infrastructure
- [ ] Handle transfer failures and retries
- [ ] Verify checksums after transfer
- [ ] Update local database on successful transfer

### Prefetch Policies
- [ ] Implement eager prefetch for thumbnails
- [ ] Implement on-demand fetch for large sidecars (proxies)
- [ ] Add bandwidth-aware policies
- [ ] Respect device storage limits
- [ ] User-configurable prefetch settings

### Resolution Integration
- [ ] Update `SdPathResolver` to check remote availability
- [ ] Implement fetch-or-generate decision logic
- [ ] Add device preference strategies (fetch vs generate)
- [ ] Handle offline devices gracefully

## Acceptance Criteria

### Basic Sync
- [ ] Devices exchange availability information periodically
- [ ] Availability table stays current across devices
- [ ] Can query which devices have which sidecars
- [ ] Availability survives device restarts

### Transfer
- [ ] Missing sidecars can be fetched from remote devices
- [ ] Transfer reuses existing file sharing protocol
- [ ] Transfers are verified with checksums
- [ ] Failed transfers can be retried
- [ ] Multiple sources supported (fetch from fastest)

### Resolution
- [ ] Resolver checks remote availability
- [ ] Can fetch sidecars from paired devices
- [ ] Respects device-specific strategies
- [ ] Falls back to local generation if fetch fails

### Performance
- [ ] Availability exchange completes in <1s
- [ ] Transfers saturate available bandwidth
- [ ] Prefetch doesn't impact foreground operations
- [ ] Minimal overhead on mobile devices

## Example Workflows

### Scenario 1: Desktop Generates, Mobile Fetches

```rust
// Desktop: User indexes photos
// → ThumbnailJob generates grid@2x thumbnails
// → Records in database: status=ready
// → Updates availability: desktop_device has grid@2x

// Sync happens (periodic, every 5 minutes)
// → Desktop sends availability digest to mobile
// → Mobile updates sidecar_availability table

// Mobile: User opens photo grid
// → Requests sidecar://550e8400.../thumbs/grid@2x.webp
// → Resolver checks local: missing
// → Resolver checks availability: desktop has it
// → Resolver returns Remote(desktop_device, path)
// → UI triggers fetch from desktop
// → Thumbnail transferred via P2P
// → Mobile updates: status=ready, has=true
```

### Scenario 2: Multiple Devices, Optimal Source

```rust
// Content exists on: MacBook (WiFi), Home Server (ethernet), Cloud (internet)
// All have the thumbnail

// Resolver evaluates sources:
// - MacBook: 45 MB/s, latency 2ms (local WiFi)
// - Home Server: 110 MB/s, latency 1ms (local ethernet)
// - Cloud: 10 MB/s, latency 50ms (internet)

// Selects Home Server (fastest + lowest latency)
// Fetches thumbnail in ~0.5ms
```

## Implementation Notes

### Availability Digest Structure

```rust
pub struct AvailabilityDigest {
    /// Device that owns this digest
    pub device_id: Uuid,

    /// Timestamp of digest creation
    pub timestamp: DateTime<Utc>,

    /// Compact representation of available sidecars
    /// For large sets, could use bloom filter
    pub sidecars: Vec<SidecarAvailabilityEntry>,
}

pub struct SidecarAvailabilityEntry {
    pub content_uuid: Uuid,
    pub kind: SidecarKind,
    pub variant: SidecarVariant,
    pub size: u64,
    pub checksum: Option<String>,
}
```

### Transfer Job

```rust
#[derive(Job)]
pub struct SidecarTransferJob {
    pub content_uuid: Uuid,
    pub kind: SidecarKind,
    pub variant: SidecarVariant,
    pub format: SidecarFormat,
    pub source_device: Uuid,
}

impl SidecarTransferJob {
    async fn execute(&self, ctx: JobContext) -> Result<()> {
        // 1. Request sidecar from source device
        let sidecar_path = SdPath::sidecar(
            self.content_uuid,
            self.kind,
            self.variant,
            self.format,
        );

        let source_physical = ctx.resolver.resolve_on_device(
            sidecar_path,
            self.source_device,
        ).await?;

        // 2. Compute local destination
        let dest_path = ctx.sidecar_manager.compute_path(...)?;

        // 3. Transfer file via P2P
        ctx.file_transfer.execute(
            source_physical,
            SdPath::Physical {
                device_slug: ctx.current_device_slug(),
                path: dest_path.absolute_path,
            },
            TransferMode::VerifyChecksum,
        ).await?;

        // 4. Record locally
        let size = fs::metadata(&dest_path.absolute_path).await?.len();
        ctx.sidecar_manager.record_sidecar(..., size, checksum).await?;

        Ok(())
    }
}
```

### Prefetch Policy

```rust
pub struct SidecarPrefetchPolicy {
    /// Always prefetch these kinds
    pub eager_kinds: HashSet<SidecarKind>,

    /// Prefetch these only when requested
    pub lazy_kinds: HashSet<SidecarKind>,

    /// Never prefetch (too large)
    pub never_prefetch: HashSet<SidecarKind>,

    /// Maximum concurrent prefetch jobs
    pub max_concurrent: usize,
}

impl Default for SidecarPrefetchPolicy {
    fn default() -> Self {
        Self {
            eager_kinds: hashset![SidecarKind::Thumb],
            lazy_kinds: hashset![SidecarKind::Ocr, SidecarKind::Transcript],
            never_prefetch: hashset![SidecarKind::Proxy],
            max_concurrent: 3,
        }
    }
}
```

## Timeline

Estimated: 1 week focused work

- Day 1-2: Availability exchange protocol
- Day 3-4: Transfer job implementation
- Day 5: Prefetch policies and strategies
- Day 6: Resolution integration
- Day 7: Testing and optimization

## Testing Strategy

```rust
#[tokio::test]
async fn test_cross_device_sidecar_availability() {
    let (alice, bob) = create_paired_devices().await;

    // Alice generates thumbnail
    let content_uuid = index_image_on_alice().await;
    generate_thumbnail(alice, content_uuid, "grid@2x").await;

    // Sync availability
    exchange_availability_digests(alice, bob).await;

    // Bob should know Alice has the thumbnail
    let availability = bob.sidecar_manager.get_presence(
        &bob.library,
        &[content_uuid],
        &SidecarKind::Thumb,
        &["grid@2x"],
    ).await?;

    assert!(availability[&content_uuid]["grid@2x"].devices.contains(&alice.device_id));
}

#[tokio::test]
async fn test_cross_device_sidecar_fetch() {
    let (alice, bob) = create_paired_devices().await;

    // Alice has thumbnail, Bob doesn't
    let content_uuid = setup_thumbnail_on_alice(alice).await;
    sync_availability(alice, bob).await;

    // Bob requests thumbnail
    let sidecar = SdPath::sidecar(content_uuid, Thumb, "grid@2x", Webp);
    let resolved = bob.resolver.resolve(sidecar).await?;

    // Should resolve to Alice
    assert!(matches!(resolved, ResolvedPath::Remote { device_id, .. } if device_id == alice.id));

    // Fetch it
    let bytes = bob.fetch_sidecar(resolved).await?;

    // Verify thumbnail
    assert_eq!(bytes.len(), 45_000); // ~45KB thumbnail
    assert!(is_valid_webp(&bytes));

    // Bob should now have it locally
    let resolved2 = bob.resolver.resolve(sidecar).await?;
    assert!(matches!(resolved2, ResolvedPath::Local { .. }));
}
```
