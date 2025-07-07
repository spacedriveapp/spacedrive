//! Derive macros for automatic job registration

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct};

/// Derive macro for automatic job registration
/// 
/// This macro generates the necessary code to automatically register a job type
/// with the job registry using the `inventory` crate.
/// 
/// Usage:
/// ```rust
/// use spacedrive_jobs_derive::Job;
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
        Data::Struct(DataStruct { .. }) => {},
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "Job can only be derived for structs"
            ).to_compile_error().into();
        }
    };

    let expanded = quote! {
        // Auto-register the job using inventory
        inventory::submit! {
            crate::infrastructure::jobs::types::JobRegistration {
                name: <#name as crate::infrastructure::jobs::traits::Job>::NAME,
                schema_fn: <#name as crate::infrastructure::jobs::traits::Job>::schema,
                create_fn: |data| {
                    let job: #name = serde_json::from_value(data)?;
                    Ok(Box::new(job))
                },
                deserialize_fn: |data| {
                    let job: #name = rmp_serde::from_slice(data)?;
                    Ok(Box::new(job))
                },
            }
        }
        
        // Implement ErasedJob for the job type
        impl crate::infrastructure::jobs::types::ErasedJob for #name {
            fn create_executor(
                self: Box<Self>,
                job_id: crate::infrastructure::jobs::types::JobId,
                library: std::sync::Arc<crate::library::Library>,
                job_db: std::sync::Arc<crate::infrastructure::jobs::database::JobDb>,
                status_tx: tokio::sync::watch::Sender<crate::infrastructure::jobs::types::JobStatus>,
                progress_tx: tokio::sync::mpsc::UnboundedSender<crate::infrastructure::jobs::progress::Progress>,
                broadcast_tx: tokio::sync::broadcast::Sender<crate::infrastructure::jobs::progress::Progress>,
                checkpoint_handler: std::sync::Arc<dyn crate::infrastructure::jobs::context::CheckpointHandler>,
                networking: Option<std::sync::Arc<crate::services::networking::NetworkingService>>,
                volume_manager: Option<std::sync::Arc<crate::volume::VolumeManager>>,
            ) -> Box<dyn sd_task_system::Task<crate::infrastructure::jobs::error::JobError>> {
                Box::new(crate::infrastructure::jobs::executor::JobExecutor::new(
                    *self,
                    job_id,
                    library,
                    job_db,
                    status_tx,
                    progress_tx,
                    broadcast_tx,
                    checkpoint_handler,
                    networking,
                    volume_manager,
                ))
            }
            
            fn serialize_state(&self) -> Result<Vec<u8>, crate::infrastructure::jobs::error::JobError> {
                rmp_serde::to_vec(self)
                    .map_err(|e| crate::infrastructure::jobs::error::JobError::serialization(format!("{}", e)))
            }
        }
    };

    TokenStream::from(expanded)
}