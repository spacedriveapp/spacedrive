---
id: CORE-014
title: Specta Codegen for Resource Events
status: To Do
assignee: unassigned
parent: CORE-011
priority: High
tags: [codegen, specta, typescript, swift]
depends_on: [CORE-011]
---

## Description

Extend the existing specta codegen system to auto-generate resource type registries for TypeScript and Swift. This ensures client-side type registries stay in sync with Rust domain models.

## Implementation Steps

1. Update `xtask/src/specta_gen.rs` to collect all `Identifiable` types
2. Generate TypeScript `resourceTypeMap` with all resource types
3. Generate Swift `ResourceTypeRegistry+Generated.swift` with registrations
4. Add build verification that all Identifiable types are registered
5. Update CI to regenerate on every commit
6. Document regeneration process for developers

## Generated Output

### TypeScript
```typescript
// packages/client/src/bindings/resourceRegistry.ts
export const resourceTypeMap = {
  'file': File,
  'album': Album,
  'tag': Tag,
  'location': Location,
  'device': Device,
  'volume': Volume,
  'content_identity': ContentIdentity,
  // ... all Identifiable types
} as const;
```

### Swift (Future)
```swift
// SpacedriveCore/Generated/ResourceTypeRegistry+Generated.swift
extension ResourceTypeRegistry {
    static func registerAllTypes() {
        register(File.self)
        register(Album.self)
        register(Tag.self)
        // ... all Identifiable types
    }
}
```

## Technical Details

- Location: `xtask/src/specta_gen.rs`
- Trait marker: Check for `impl Identifiable`
- Output: `packages/client/src/bindings/resourceRegistry.ts`
- Build step: `cargo xtask specta-gen`
- CI: Auto-run on pre-commit or CI build

## Acceptance Criteria

- [ ] Specta codegen extended for resource types
- [ ] TypeScript resourceTypeMap auto-generated
- [ ] Build verification ensures all types registered
- [ ] CI/CD regenerates on every commit
- [ ] Developer documentation updated
- [ ] Diff checking prevents manual edits

## References

- `docs/core/events.md` lines 391-434
- Existing: `xtask/src/specta_gen.rs`
