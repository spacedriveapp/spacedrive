//! Spacedrive SDK Macros
//!
//! Proc macros that make extension development delightful.

use proc_macro::TokenStream;

mod extension;
mod job;

/// Main job macro - makes job definition beautiful
///
/// # Example
///
/// ```no_run
/// #[job]
/// async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
///     for email in fetch_emails(&state.last_uid)? {
///         ctx.check()?;  // Auto-checkpoints!
///         process_email(ctx, email).await?;
///         state.last_uid = email.uid;
///     }
///     Ok(())
/// }
/// ```
///
/// Generates:
/// - FFI export: `extern "C" fn execute_email_scan(...) -> i32`
/// - State marshalling
/// - Error handling
/// - Auto-checkpoint on interrupt
#[proc_macro_attribute]
pub fn job(args: TokenStream, input: TokenStream) -> TokenStream {
	job::job_impl(args, input)
}

/// Extension container macro
///
/// # Example
///
/// ```no_run
/// #[extension(
///     id = "finance",
///     name = "Spacedrive Finance",
///     version = "0.1.0"
/// )]
/// struct Finance;
/// ```
///
/// Generates:
/// - plugin_init() and plugin_cleanup()
/// - Manifest generation (build.rs)
/// - Registration code
#[proc_macro_attribute]
pub fn extension(args: TokenStream, input: TokenStream) -> TokenStream {
	extension::extension_impl(args, input)
}

/// Query macro (future)
#[proc_macro_attribute]
pub fn spacedrive_query(_args: TokenStream, input: TokenStream) -> TokenStream {
	// TODO: Implement
	input
}

/// Action macro (future)
#[proc_macro_attribute]
pub fn spacedrive_action(_args: TokenStream, input: TokenStream) -> TokenStream {
	// TODO: Implement
	input
}
