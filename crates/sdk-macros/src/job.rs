//! Job macro implementation

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Type};

pub fn spacedrive_job_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let input_fn = parse_macro_input!(input as ItemFn);

	// Extract function info
	let fn_name = &input_fn.sig.ident;
	let fn_attrs = &input_fn.attrs;

	// Generate FFI export name
	let export_name = syn::Ident::new(&format!("execute_{}", fn_name), fn_name.span());

	// Extract state type from second parameter
	// Expected signature: async fn name(ctx: &JobContext, state: &mut State) -> Result<()>
	let state_type = extract_state_type(&input_fn);

	let expanded = quote! {
		// Keep original function for internal use
		#(#fn_attrs)*
		#input_fn

		// Generate FFI export
		#[no_mangle]
		pub extern "C" fn #export_name(
			ctx_json_ptr: u32,
			ctx_json_len: u32,
			state_json_ptr: u32,
			state_json_len: u32,
		) -> i32 {
			// Parse job context
			let ctx_json = unsafe {
				let slice = ::std::slice::from_raw_parts(
					ctx_json_ptr as *const u8,
					ctx_json_len as usize
				);
				::std::str::from_utf8(slice).unwrap_or("{}")
			};

			let job_ctx = match ::spacedrive_sdk::job_context::JobContext::from_params(ctx_json) {
				Ok(ctx) => ctx,
				Err(e) => {
					::spacedrive_sdk::ffi::log_error(&format!("Failed to parse job context: {}", e));
					return ::spacedrive_sdk::job_context::JobResult::Failed("Invalid context".into()).to_exit_code();
				}
			};

			// Load or initialize state
			let mut state: #state_type = if state_json_len > 0 {
				let state_json = unsafe {
					let slice = ::std::slice::from_raw_parts(
						state_json_ptr as *const u8,
						state_json_len as usize
					);
					::std::str::from_utf8(slice).unwrap_or("{}")
				};

				match ::serde_json::from_str(state_json) {
					Ok(s) => s,
					Err(e) => {
						job_ctx.log_error(&format!("Failed to deserialize state: {}", e));
						return ::spacedrive_sdk::job_context::JobResult::Failed("Invalid state".into()).to_exit_code();
					}
				}
			} else {
				<#state_type>::default()
			};

			// Execute user's function
			let result = #fn_name(&job_ctx, &mut state);

			// Handle result
			match result {
				Ok(_) => {
					job_ctx.log(&format!("Job {} completed successfully", stringify!(#fn_name)));
					::spacedrive_sdk::job_context::JobResult::Completed.to_exit_code()
				}
				Err(e) => {
					// Check if it's an interrupt
					let error_str = e.to_string();
					if error_str.contains("interrupt") || error_str.contains("Interrupt") {
						job_ctx.log("Job interrupted, checkpoint saved");
						let _ = job_ctx.checkpoint(&state);
						::spacedrive_sdk::job_context::JobResult::Interrupted.to_exit_code()
					} else {
						job_ctx.log_error(&format!("Job failed: {}", e));
						::spacedrive_sdk::job_context::JobResult::Failed(error_str).to_exit_code()
					}
				}
			}
		}
	};

	TokenStream::from(expanded)
}

fn extract_state_type(input_fn: &ItemFn) -> Type {
	// Get second parameter (state: &mut State)
	if let Some(FnArg::Typed(pat_type)) = input_fn.sig.inputs.iter().nth(1) {
		// Extract the inner type from &mut T
		if let Type::Reference(type_ref) = &*pat_type.ty {
			if let Type::Path(type_path) = &*type_ref.elem {
				return Type::Path(type_path.clone());
			}
		}
	}

	// Fallback to generic type
	syn::parse_quote!(::serde_json::Value)
}
