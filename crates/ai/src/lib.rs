use thiserror::Error;

use ort::EnvironmentBuilder;
use tracing::{debug, error};

pub mod old_image_labeler;
mod utils;

// This path must be relative to the running binary
#[cfg(target_os = "windows")]
const BINDING_LOCATION: &str = ".";

#[cfg(target_os = "macos")]
const BINDING_LOCATION: &str = "../Frameworks/Spacedrive.framework/Libraries";

#[cfg(target_os = "windows")]
const LIB_NAME: &str = "onnxruntime.dll";

#[cfg(any(target_os = "macos", target_os = "ios"))]
const LIB_NAME: &str = "libonnxruntime.dylib";

pub fn init() -> Result<(), Error> {
	#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
	{
		use std::path::Path;
		let path = utils::get_path_relative_to_exe(Path::new(BINDING_LOCATION).join(LIB_NAME));
		std::env::set_var("ORT_DYLIB_PATH", path);
	}

	// Initialize AI stuff
	EnvironmentBuilder::default()
		.with_name("spacedrive")
		.with_execution_providers({
			#[cfg(any(target_os = "macos", target_os = "ios"))]
			{
				use ort::{CoreMLExecutionProvider, XNNPACKExecutionProvider};

				[
					CoreMLExecutionProvider::default().build(),
					XNNPACKExecutionProvider::default().build(),
				]
			}

			#[cfg(target_os = "windows")]
			{
				use ort::DirectMLExecutionProvider;

				[DirectMLExecutionProvider::default().build()]
			}

			#[cfg(target_os = "linux")]
			{
				use ort::XNNPACKExecutionProvider;

				[XNNPACKExecutionProvider::default().build()]
			}

			// #[cfg(target_os = "android")]
			// {
			// 	use ort::{
			// 		ACLExecutionProvider, ArmNNExecutionProvider, NNAPIExecutionProvider,
			// 		QNNExecutionProvider, XNNPACKExecutionProvider,
			// 	};
			// 	[
			// 		QNNExecutionProvider::default().build(),
			// 		NNAPIExecutionProvider::default().build(),
			// 		XNNPACKExecutionProvider::default().build(),
			// 		ACLExecutionProvider::default().build(),
			// 		ArmNNExecutionProvider::default().build(),
			// 	]
			// }
		})
		.commit()?;

	debug!("Initialized AI environment");

	Ok(())
}

#[derive(Error, Debug)]
pub enum Error {
	#[error("failed to initialize AI environment: {0}")]
	Init(#[from] ort::Error),
	#[error(transparent)]
	ImageLabeler(#[from] old_image_labeler::ImageLabelerError),
}
