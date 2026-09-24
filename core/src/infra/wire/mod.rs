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
pub mod u64_string;

// Re-export commonly used items
pub use api_types::{ApiJobHandle, ToApiType};
pub use registry::{
	handle_core_action, handle_core_query, handle_library_action, handle_library_preview,
	handle_library_query, handle_library_validate, CoreActionEntry, CoreQueryEntry,
	LibraryActionEntry, LibraryPreviewEntry, LibraryQueryEntry, LibraryValidateEntry, CORE_ACTIONS,
	CORE_QUERIES, LIBRARY_ACTIONS, LIBRARY_PREVIEWS, LIBRARY_QUERIES, LIBRARY_VALIDATES,
};
pub use type_extraction::{
	create_spacedrive_api_structure, generate_spacedrive_api, OperationScope, OperationTypeInfo,
	QueryScope, QueryTypeInfo,
};

/// Links an operation's input type to the daemon method name it is
/// dispatched under. Implemented by the `register_*` macros; clients use
/// the associated constant to address queries and actions over the wire.
pub trait Wire {
	const METHOD: &'static str;
}

/// The two questions an action answers before it runs, each its own wire
/// method over the action's input: `validate:<name>` and `preview:<name>`
/// beside `action:<name>.input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preflight {
	Validate,
	Preview,
}

impl Preflight {
	pub fn prefix(self) -> &'static str {
		match self {
			Self::Validate => "validate",
			Self::Preview => "preview",
		}
	}
}

/// An action's short name from its wire method: `files.merge` from
/// `action:files.merge.input`. `None` for a method that is not an action's.
pub fn action_name(method: &str) -> Option<&str> {
	method.strip_prefix("action:")?.strip_suffix(".input")
}

/// The wire method of a preflight check over an action: `validate:files.merge` for
/// `action:files.merge.input`.
pub fn preflight_method(action_method: &str, preflight: Preflight) -> Option<String> {
	action_name(action_method).map(|name| format!("{}:{name}", preflight.prefix()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_rail_is_addressed_by_its_actions_name() {
		assert_eq!(action_name("action:files.merge.input"), Some("files.merge"));
		assert_eq!(action_name("query:paths.compare"), None);
		assert_eq!(
			preflight_method("action:files.merge.input", Preflight::Validate).as_deref(),
			Some("validate:files.merge")
		);
		assert_eq!(
			preflight_method("action:files.merge.input", Preflight::Preview).as_deref(),
			Some("preview:files.merge")
		);
		assert_eq!(
			preflight_method("query:paths.compare", Preflight::Preview),
			None
		);
	}
}
