//! Derive macros for automatic job registration

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput};

/// Derive macro for automatic job registration
///
/// This macro generates the necessary code to automatically register a job type
/// with the job registry using the `inventory` crate.
///
/// Usage:
/// ```rust
/// use job_derive::Job;
///
/// #[derive(Job, Serialize, Deserialize)]
/// pub struct MyJob {
///     // job fields
/// }
///
/// impl JobHandler for MyJob {
///     // implementation
/// }
/// ```
#[proc_macro_derive(Job)]
pub fn derive_job(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	// Ensure this is a struct
	let _data = match &input.data {
		Data::Struct(DataStruct { .. }) => {}
		_ => {
			return syn::Error::new_spanned(&input.ident, "Job can only be derived for structs")
				.to_compile_error()
				.into();
		}
	};

	let expanded = quote! {
		// Auto-register the job using inventory
		inventory::submit! {
			crate::infra::job::types::JobRegistration {
				name: <#name as crate::infra::job::traits::Job>::NAME,
				schema_fn: <#name as crate::infra::job::traits::Job>::schema,
				create_fn: |data| {
					let job: #name = serde_json::from_value(data)?;
					Ok(Box::new(job))
				},
				deserialize_fn: |data| {
					let job: #name = rmp_serde::from_slice(data)?;
					Ok(Box::new(job))
				},
				deserialize_dyn_fn: |data| {
					let job: #name = rmp_serde::from_slice(data)?;
					Ok(Box::new(job))
				},
			}
		}

		// Implement ErasedJob for the job type
		impl crate::infra::job::types::ErasedJob for #name {
			fn create_executor(
				self: Box<Self>,
				job_id: crate::infra::job::types::JobId,
				library: std::sync::Arc<crate::library::Library>,
				job_db: std::sync::Arc<crate::infra::job::database::JobDb>,
				status_tx: tokio::sync::watch::Sender<crate::infra::job::types::JobStatus>,
				progress_tx: tokio::sync::mpsc::UnboundedSender<crate::infra::job::progress::Progress>,
				broadcast_tx: tokio::sync::broadcast::Sender<crate::infra::job::progress::Progress>,
				checkpoint_handler: std::sync::Arc<dyn crate::infra::job::context::CheckpointHandler>,
				output_handle: std::sync::Arc<tokio::sync::Mutex<Option<Result<crate::infra::job::output::JobOutput, crate::infra::job::error::JobError>>>>,
				networking: Option<std::sync::Arc<crate::service::network::NetworkingService>>,
				volume_manager: Option<std::sync::Arc<crate::volume::VolumeManager>>,
				job_logging_config: Option<crate::config::JobLoggingConfig>,
				job_logs_dir: Option<std::path::PathBuf>,
			) -> Box<dyn sd_task_system::Task<crate::infra::job::error::JobError>> {
				Box::new(crate::infra::job::executor::JobExecutor::new(
					*self,
					job_id,
					library,
					job_db,
					status_tx,
					progress_tx,
					broadcast_tx,
					checkpoint_handler,
					output_handle,
					networking,
					volume_manager,
					job_logging_config,
					job_logs_dir,
				))
			}

			fn serialize_state(&self) -> Result<Vec<u8>, crate::infra::job::error::JobError> {
				rmp_serde::to_vec(self)
					.map_err(|e| crate::infra::job::error::JobError::serialization(format!("{}", e)))
			}
		}
	};

	TokenStream::from(expanded)
}

