//! Extension container macro implementation

use proc_macro::TokenStream;
use quote::quote;
use syn::{
	parse::{Parse, ParseStream},
	parse_macro_input, Expr, Ident, ItemStruct, LitStr, Result, Token,
};

struct ExtensionArgs {
	id: String,
	name: String,
	version: String,
	jobs: Vec<Ident>,
	// We'll ignore other parameters for now (description, permissions, etc.)
	// They can be used by tooling but don't need codegen
}

impl Parse for ExtensionArgs {
	fn parse(input: ParseStream) -> Result<Self> {
		let mut id = None;
		let mut name = None;
		let mut version = None;
		let mut jobs = Vec::new();

		while !input.is_empty() {
			let ident: Ident = input.parse()?;
			input.parse::<Token![=]>()?;

			match ident.to_string().as_str() {
				"id" => {
					let lit: LitStr = input.parse()?;
					id = Some(lit.value());
				}
				"name" => {
					let lit: LitStr = input.parse()?;
					name = Some(lit.value());
				}
				"version" => {
					let lit: LitStr = input.parse()?;
					version = Some(lit.value());
				}
				"jobs" => {
					let content;
					syn::bracketed!(content in input);
					while !content.is_empty() {
						jobs.push(content.parse()?);
						if content.peek(Token![,]) {
							content.parse::<Token![,]>()?;
						}
					}
				}
				// Ignore other parameters - just skip their values
				"description" | "min_core_version" => {
					let _: LitStr = input.parse()?;
				}
				"required_features" | "permissions" => {
					// Parse array but ignore
					let content;
					syn::bracketed!(content in input);
					while !content.is_empty() {
						let _: Expr = content.parse()?;
						if content.peek(Token![,]) {
							content.parse::<Token![,]>()?;
						}
					}
				}
				_ => {
					// Unknown parameter - try to skip it
					// This is a best-effort approach
					if input.peek(syn::token::Bracket) {
						let content;
						syn::bracketed!(content in input);
						// Consume everything in brackets
						while !content.is_empty() {
							let _: proc_macro2::TokenStream = content.parse()?;
							if content.peek(Token![,]) {
								content.parse::<Token![,]>()?;
							}
						}
					} else {
						// Try to parse as expression and discard
						let _: Expr = input.parse()?;
					}
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		Ok(ExtensionArgs {
			id: id.ok_or_else(|| input.error("missing id parameter"))?,
			name: name.ok_or_else(|| input.error("missing name parameter"))?,
			version: version.ok_or_else(|| input.error("missing version parameter"))?,
			jobs,
		})
	}
}

pub fn extension_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	let input_struct = parse_macro_input!(input as ItemStruct);
	let args = parse_macro_input!(args as ExtensionArgs);

	let ext_id = &args.id;
	let ext_name = &args.name;
	let ext_version = &args.version;

	// Generate job registration code
	let job_registrations = args.jobs.iter().map(|job_fn| {
		let register_fn = quote::format_ident!("__register_{}", job_fn);
		quote! {
			{
				let (name, export_fn, resumable) = #register_fn();
				::spacedrive_sdk::ffi::log_info(&format!("Registering job: {}", name));

				if let Err(_) = ::spacedrive_sdk::ffi::register_job_with_host(
					name,
					export_fn,
					resumable
				) {
					::spacedrive_sdk::ffi::log_error(&format!("Failed to register job: {}", name));
					return 1;
				}
			}
		}
	});

	let expanded = quote! {
		#input_struct

		// Generate plugin_init with auto-registration
		#[no_mangle]
		pub extern "C" fn plugin_init() -> i32 {
			::spacedrive_sdk::ffi::log_info(&format!(
				"{} v{} initializing...",
				#ext_name,
				#ext_version
			));

			// Register all jobs
			#(#job_registrations)*

			::spacedrive_sdk::ffi::log_info(&format!(
				"✓ {} v{} initialized!",
				#ext_name,
				#ext_version
			));

			0 // Success
		}

		// Generate plugin_cleanup
		#[no_mangle]
		pub extern "C" fn plugin_cleanup() -> i32 {
			::spacedrive_sdk::ffi::log_info(&format!(
				"{} cleanup",
				#ext_name
			));
			0 // Success
		}
	};

	TokenStream::from(expanded)
}
