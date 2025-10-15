//! Task macro implementation

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub fn task_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	// For now, just pass through
	// Future: Generate task wrapper with retry logic, timeout, etc.
	let expanded = quote! {
		#input
	};

	TokenStream::from(expanded)
}
