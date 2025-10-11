//! Agent and agent_memory macro implementations

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemImpl};

pub fn agent_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemImpl);

	// For now, just pass through
	// Future: Generate event handler registration, lifecycle hooks, etc.
	let expanded = quote! {
		#input
	};

	TokenStream::from(expanded)
}

pub fn agent_memory_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	// Generate AgentMemory trait impl
	let expanded = quote! {
		#input

		impl ::spacedrive_sdk::agent::AgentMemory for #name {}
	};

	TokenStream::from(expanded)
}

