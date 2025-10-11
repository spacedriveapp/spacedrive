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
