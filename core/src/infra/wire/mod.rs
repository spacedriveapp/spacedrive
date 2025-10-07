//! Wire protocol and type system infrastructure
//!
//! This module contains the plumbing that connects client applications
//! to core operations via Unix domain sockets:
//!
//! ## Components
//!
//! - **Registry**: Compile-time registration using `inventory` crate,
//!   maps method strings to handler functions
//! - **Type Extraction**: Generates client types (Swift, TypeScript) from
//!   Rust types using Specta
//! - **API Types**: Wrappers for client-compatible types (e.g., ApiJobHandle)
//!
//! ## How It Works
//!
//! 1. Operations register with macros: `register_library_query!`, etc.
//! 2. At compile time, `inventory` collects all registrations
//! 3. At runtime, daemon looks up handlers by method string
//! 4. Handlers deserialize input, execute operation, serialize output
//! 5. At build time, code generators use type extractors to create clients

pub mod api_types;
pub mod registry;
#[cfg(test)]
pub mod test_type_extraction;
pub mod type_extraction;

// Re-export commonly used items
pub use api_types::{ApiJobHandle, ToApiType};
pub use registry::{
	handle_core_action, handle_core_query, handle_library_action, handle_library_query,
	CoreActionEntry, CoreQueryEntry, LibraryActionEntry, LibraryQueryEntry, CORE_ACTIONS,
	CORE_QUERIES, LIBRARY_ACTIONS, LIBRARY_QUERIES,
};
pub use type_extraction::{
	create_spacedrive_api_structure, generate_spacedrive_api, OperationScope, OperationTypeInfo,
	QueryScope, QueryTypeInfo,
};
