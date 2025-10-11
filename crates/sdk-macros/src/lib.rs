//! Spacedrive SDK Macros
//!
//! Proc macros that make extension development delightful.

use proc_macro::TokenStream;

mod action;
mod agent;
mod extension;
mod job;
mod model;
mod query;
mod task;

/// Extension container macro
///
/// Generates plugin_init() and plugin_cleanup() exports
#[proc_macro_attribute]
pub fn extension(args: TokenStream, input: TokenStream) -> TokenStream {
	extension::extension_impl(args, input)
}

/// Job macro - generates FFI exports and state marshalling
///
/// Generates:
/// - FFI export: `extern "C" fn execute_<name>(...) -> i32`
/// - State serialization/deserialization
/// - Error handling with auto-checkpoint on interrupt
#[proc_macro_attribute]
pub fn job(args: TokenStream, input: TokenStream) -> TokenStream {
	job::job_impl(args, input)
}

/// Model macro - generates ExtensionModel trait impl
///
/// For extension data models (Person, Album, Place)
/// NOT for AI/ML models (those go in ai.rs)
#[proc_macro_attribute]
pub fn model(args: TokenStream, input: TokenStream) -> TokenStream {
	model::model_impl(args, input)
}

/// Agent macro - marks agent implementation
///
/// Future: Will generate event handler registration
#[proc_macro_attribute]
pub fn agent(args: TokenStream, input: TokenStream) -> TokenStream {
	agent::agent_impl(args, input)
}

/// Agent memory macro - generates AgentMemory trait impl
#[proc_macro_attribute]
pub fn agent_memory(args: TokenStream, input: TokenStream) -> TokenStream {
	agent::agent_memory_impl(args, input)
}

/// Action macro - marks action function
///
/// Future: Will generate FFI exports for preview/execute
#[proc_macro_attribute]
pub fn action(args: TokenStream, input: TokenStream) -> TokenStream {
	action::action_impl(args, input)
}

/// Query macro - marks query function
///
/// Future: Will generate FFI exports for query registration
#[proc_macro_attribute]
pub fn query(args: TokenStream, input: TokenStream) -> TokenStream {
	query::query_impl(args, input)
}

/// Task macro - marks task function
///
/// Future: Will generate task wrapper with retry/timeout
#[proc_macro_attribute]
pub fn task(args: TokenStream, input: TokenStream) -> TokenStream {
	task::task_impl(args, input)
}

/// Agent trail macro - configures agent logging
#[proc_macro_attribute]
pub fn agent_trail(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// On startup handler macro
#[proc_macro_attribute]
pub fn on_startup(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// On event handler macro
#[proc_macro_attribute]
pub fn on_event(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Scheduled task macro
#[proc_macro_attribute]
pub fn scheduled(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Filter attribute for event handlers
#[proc_macro_attribute]
pub fn filter(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Action execute macro
#[proc_macro_attribute]
pub fn action_execute(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Agent memory config macro
#[proc_macro_attribute]
pub fn memory_config(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Persist strategy attribute
#[proc_macro_attribute]
pub fn persist_strategy(_args: TokenStream, input: TokenStream) -> TokenStream {
	input // Pass through for now
}

/// Setting attribute for config fields
///
/// Note: This is a helper attribute that gets processed by the struct-level macros.
/// It provides metadata for Spacedrive's configuration UI.
/// In the current stub implementation, it's used for documentation purposes.
#[proc_macro_attribute]
pub fn setting(_args: TokenStream, input: TokenStream) -> TokenStream {
	// This is a marker attribute that gets stripped during compilation
	// Real implementation would be processed by a derive macro on the parent struct
	input
}
