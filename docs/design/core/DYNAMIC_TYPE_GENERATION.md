# Dynamic Type Generation: The rspc Magic Applied to Spacedrive

## Overview

This document explains how to implement truly dynamic type-safe API generation for Spacedrive's Swift client by applying the techniques pioneered by the rspc library. The goal is to automatically generate complete Swift API enums with actual type references, eliminating the need for manual type registration or hardcoding.

## The Problem: Compile-Time vs Runtime Type Collection

### Current Approach (Doesn't Work)

Our initial attempts tried to use the inventory system to dynamically generate enum variants:

```rust
#[macro_export]
macro_rules! generate_inventory_enums {
    () => {
        use $crate::ops::registry::{TYPED_ACTIONS, TYPED_QUERIES};

        // FAILS: This tries to iterate at compile-time over runtime data
        for action in TYPED_ACTIONS.iter() {
            // Generate enum variants...
        }
    };
}
```

**Why This Fails:**

1. **Macro Expansion Time** (compile-time): `generate_inventory_enums!()` tries to expand
2. **Compilation Time** (compile-time): Rust compiles `inventory::submit!` calls
3. **Runtime**: `TYPED_ACTIONS` gets populated via `Lazy::new()`

The fundamental issue: **inventory collects data at runtime, but macros expand at compile-time**.

### The Timeline Problem

```
┌─ COMPILE TIME ─────────────────────────────────┐
│ 1. Macro expansion                             │
│    - generate_inventory_enums!() needs data   │
│    - But TYPED_ACTIONS doesn't exist yet!     │
│                                                │
│ 2. Code compilation                            │
│    - inventory::submit! calls compile         │
│    - Static data structures created           │
│    - But no way to iterate at compile-time    │
└────────────────────────────────────────────────┘

┌─ RUNTIME ──────────────────────────────────────┐
│ 3. Program execution                           │
│    - TYPED_ACTIONS populated via Lazy::new()  │
│    - inventory::iter() finally works          │
│    - But enum was already compiled!           │
└────────────────────────────────────────────────┘
```

## The rspc Solution: Trait-Based Type Extraction

### How rspc Solves This

rspc uses **generic functions with type constraints** and **automatic trait implementations** to extract types at compile-time:

#### 1. Generic Registration Functions

```rust
pub fn query<TResolver, TArg, TResult, TResultMarker>(
    mut self,
    key: &'static str,
    builder: impl Fn(UnbuiltProcedureBuilder<TLayerCtx, TResolver>) -> BuiltProcedureBuilder<TResolver>,
) -> Self
where
    TArg: DeserializeOwned + Type,        // ← Input type must implement Type
    TResult: RequestLayer<TResultMarker>, // ← Output type must implement Type
    TResolver: Fn(TLayerCtx, TArg) -> TResult + Send + Sync + 'static,
{
    // The magic happens here: TResolver::typedef() is called automatically
    let type_info = TResolver::typedef(&mut self.type_map);
    // Register both the handler AND the type information
}
```

#### 2. Automatic Trait Implementation for Functions

```rust
impl<TFunc, TCtx, TArg, TResult, TResultMarker>
    Resolver<TCtx, DoubleArgMarker<TArg, TResultMarker>> for TFunc
where
    TArg: DeserializeOwned + Type,        // ← Input constraint
    TFunc: Fn(TCtx, TArg) -> TResult,     // ← Function signature constraint
    TResult: RequestLayer<TResultMarker>, // ← Output constraint
{
    fn typedef(defs: &mut TypeCollection) -> ProcedureDataType {
        typedef::<TArg, TResult::Result>(defs)  // ← AUTOMATIC extraction!
    }
}
```

#### 3. The Type Extraction Magic

```rust
pub fn typedef<TArg: Type, TResult: Type>(defs: &mut TypeCollection) -> ProcedureDataType {
    let arg_ty = TArg::reference(defs, &[]).inner;     // ← Extract input type
    let result_ty = TResult::reference(defs, &[]).inner; // ← Extract output type

    ProcedureDataType { arg_ty, result_ty }
}
```

### Key Insights from rspc

1. **No Runtime Iteration**: Types are extracted through generic constraints, not runtime loops
2. **Automatic Implementation**: Functions automatically get type extraction via trait bounds
3. **Compile-Time Type Collection**: Uses Specta's `TypeCollection` to gather types during compilation
4. **Trait-Based Discovery**: Uses traits to provide type metadata, not runtime data structures

## Applying rspc Magic to Spacedrive

### Solution: Operation Type Extraction Trait

Here's how we can implement the same approach for Spacedrive:

#### 1. Define the Core Trait

```rust
use specta::{Type, TypeCollection};
use serde::{Serialize, de::DeserializeOwned};

/// Trait that provides compile-time type information for operations
pub trait OperationTypeInfo {
    type Input: Type + Serialize + DeserializeOwned;
    type Output: Type + Serialize + DeserializeOwned;

    /// The operation identifier (e.g., "files.copy")
    fn identifier() -> &'static str;

    /// The wire method for this operation
    fn wire_method() -> String {
        format!("action:{}.input.v1", Self::identifier())
    }

    /// Extract type metadata and register with Specta
    fn extract_types(collection: &mut TypeCollection) -> OperationMetadata {
        OperationMetadata {
            identifier: Self::identifier(),
            wire_method: Self::wire_method(),
            input_type: Self::Input::reference(collection, &[]).inner,
            output_type: Self::Output::reference(collection, &[]).inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationMetadata {
    pub identifier: &'static str,
    pub wire_method: String,
    pub input_type: specta::DataType,
    pub output_type: specta::DataType,
}
```

#### 2. Enhanced Registration Macros

```rust
#[macro_export]
macro_rules! register_library_action {
    ($action:ty, $name:literal) => {
        // Existing inventory registration
        impl $crate::client::Wire for <$action as $crate::infra::action::LibraryAction>::Input {
            const METHOD: &'static str = $crate::action_method!($name);
        }

        inventory::submit! {
            $crate::ops::registry::ActionEntry {
                method: <<$action as $crate::infra::action::LibraryAction>::Input as $crate::client::Wire>::METHOD,
                handler: $crate::ops::registry::handle_library_action::<$action>,
                input_type_name: stringify!(<$action as $crate::infra::action::LibraryAction>::Input),
                output_type_name: "JobHandle",
                action_type_name: stringify!($action),
                is_library_action: true,
            }
        }

        // NEW: Automatic type extraction trait implementation
        impl $crate::ops::OperationTypeInfo for $action {
            type Input = <$action as $crate::infra::action::LibraryAction>::Input;
            type Output = $crate::infra::job::handle::JobHandle;

            fn identifier() -> &'static str {
                $name
            }
        }

        // NEW: Register the type info for compile-time collection
        inventory::submit! {
            $crate::ops::TypeExtractorEntry {
                extractor: || <$action as $crate::ops::OperationTypeInfo>::extract_types,
                identifier: $name,
            }
        }
    };
}
```

#### 3. Compile-Time Type Collection

```rust
/// Entry for compile-time type extraction
pub struct TypeExtractorEntry {
    pub extractor: fn(&mut TypeCollection) -> OperationMetadata,
    pub identifier: &'static str,
}

inventory::collect!(TypeExtractorEntry);

/// Generate complete API enums with automatic type discovery
pub fn generate_spacedrive_api() -> (Vec<OperationMetadata>, TypeCollection) {
    let mut collection = TypeCollection::default();
    let mut operations = Vec::new();

    // This WORKS because we iterate over compile-time registered extractors
    for entry in inventory::iter::<TypeExtractorEntry>() {
        let metadata = (entry.extractor)(&mut collection);
        operations.push(metadata);
    }

    (operations, collection)
}
```

#### 4. Dynamic Enum Generation

```rust
#[macro_export]
macro_rules! generate_dynamic_spacedrive_api {
    () => {
        use specta::Type;
        use serde::{Deserialize, Serialize};

        /// Dynamically generated SpacedriveAction enum
        #[derive(Debug, Clone, Type, Serialize, Deserialize)]
        pub enum SpacedriveAction {
            // Generate variants based on collected operations
            $(
                $variant_name {
                    input: $input_type,
                    output: $output_type,
                    identifier: &'static str,
                }
            ),*
        }

        impl SpacedriveAction {
            pub fn wire_method(&self) -> &str {
                match self {
                    $(
                        Self::$variant_name { identifier, .. } => {
                            // Use the wire method from metadata
                        }
                    ),*
                }
            }
        }
    };
}
```

### Benefits of This Approach

#### Compile-Time Type Safety
- All type extraction happens during compilation
- No runtime overhead for type discovery
- Impossible to have type mismatches

#### Automatic Discovery
- New operations automatically appear in Swift types
- No manual registration or hardcoding required
- Zero maintenance burden

#### Complete Type Information
- Swift gets actual Input/Output types, not string names
- Full type safety in Swift client code
- IntelliSense and compile-time checking

#### rspc-Proven Architecture
- Based on battle-tested rspc implementation
- Leverages Specta's type system properly
- Follows Rust's trait-based design patterns

## Implementation Plan

### Phase 1: Core Infrastructure
1. Define `OperationTypeInfo` trait
2. Create `TypeExtractorEntry` and inventory collection
3. Implement `generate_spacedrive_api()` function

### Phase 2: Enhanced Registration Macros
1. Update `register_library_action!` macro
2. Update `register_core_action!` macro
3. Update `register_query!` macro

### Phase 3: Dynamic Generation
1. Create proc macro for enum generation
2. Implement variant generation from metadata
3. Generate wire method implementations

### Phase 4: Swift Integration
1. Update Swift type generator
2. Generate complete Swift API enums
3. Test type safety and completeness

### Phase 5: Testing & Validation
1. Verify all 41 operations are discovered
2. Test Swift compilation and type safety
3. Validate wire method generation
4. Performance testing

## Example Usage

### Rust Side (Automatic)

```rust
// Define an action (unchanged)
pub struct FileCopyAction {
    // ... implementation
}

// Register it (enhanced macro does the magic)
register_library_action!(FileCopyAction, "files.copy");

// Generate Swift types (automatic discovery)
let (operations, type_collection) = generate_spacedrive_api();
specta_swift::export(&type_collection, "Types.swift")?;
```

### Swift Side (Generated)

```swift
// Automatically generated - no manual work needed!
public enum SpacedriveAction {
    case filesCopy(SpacedriveActionFilesCopyData)
    case librariesCreate(SpacedriveActionLibrariesCreateData)
    // ... all 29 actions automatically included
}

public struct SpacedriveActionFilesCopyData: Codable {
    public let input: FileCopyInput      // Actual Swift type!
    public let output: JobHandle         // Actual Swift type!
    public let identifier: String        // "files.copy"
}

// Type-safe usage
let copyAction = SpacedriveAction.filesCopy(SpacedriveActionFilesCopyData(
    input: FileCopyInput(/* fully typed fields */),
    output: JobHandle(/* fully typed fields */),
    identifier: "files.copy"
))
```

## Conclusion

By applying rspc's trait-based type extraction approach, we can achieve truly dynamic, type-safe API generation for Spacedrive. This eliminates the compile-time vs runtime data collection problem and provides a scalable, maintainable solution that automatically keeps Swift types in sync with Rust operations.

The key insight from rspc is: **don't try to iterate over runtime data at compile-time. Instead, use traits and generic constraints to extract type information during compilation itself.**
