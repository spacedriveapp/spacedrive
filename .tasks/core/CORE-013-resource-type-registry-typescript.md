---
id: CORE-013
title: Resource Type Registry (TypeScript)
status: To Do
assignee: jamiepine
parent: CORE-011
priority: High
tags: [client, typescript, codegen, cache]
depends_on: [CORE-011]
---

## Description

Create the TypeScript ResourceTypeRegistry for web/desktop clients. Enables generic deserialization of resource events with type safety maintained through generated bindings.

## Implementation Steps

1. Create `ResourceTypeRegistry` class
2. Implement `register()` method with validators
3. Implement `decode()` method
4. Auto-generate `resourceTypeMap` via specta
5. Add auto-registration from generated map
6. Integrate with EventCacheUpdater
7. Add TypeScript type safety

## Technical Details

- Location: `packages/client/src/core/ResourceTypeRegistry.ts`
- Auto-generated: `packages/client/src/bindings/resourceRegistry.ts`
- Type-safe: TypeScript types generated from Rust
- Validation: Runtime type checking (optional)

## Example

```typescript
class ResourceTypeRegistry {
	private static validators = new Map<string, (data: unknown) => any>();

	static register<T>(resourceType: string, validator: (data: unknown) => T) {
		this.validators.set(resourceType, validator);
	}

	static decode(resourceType: string, data: unknown): any {
		const validator = this.validators.get(resourceType);
		if (!validator) {
			throw new Error(`Unknown resource type: ${resourceType}`);
		}
		return validator(data);
	}
}

// Auto-generated from specta
export const resourceTypeMap = {
	file: File,
	album: Album,
	tag: Tag,
	location: Location,
} as const;

// Auto-registration
Object.entries(resourceTypeMap).forEach(([type, TypeClass]) => {
	ResourceTypeRegistry.register(
		type,
		(data) => data as InstanceType<typeof TypeClass>,
	);
});
```

## Acceptance Criteria

- [ ] ResourceTypeRegistry implemented
- [ ] Auto-generated resourceTypeMap from specta
- [ ] Type safety preserved through generics
- [ ] Error handling for unknown types
- [ ] Unit tests for registration and decoding
- [ ] Integration with EventCacheUpdater

## References

- `docs/core/events.md` lines 302-389
- Specta codegen: `xtask/src/specta_gen.rs`
