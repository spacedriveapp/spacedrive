//! Action macro implementation

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub fn action_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	// For now, just pass through
	// Future: Generate FFI export for action preview/execute
	let expanded = quote! {
		#input
	};

	TokenStream::from(expanded)
}


