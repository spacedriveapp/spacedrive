use std::{
	env::{args_os, current_exe},
	path::{Path, PathBuf},
};
use tracing::error;

pub(crate) fn get_path_relative_to_exe(path: impl AsRef<Path>) -> PathBuf {
	current_exe()
		.unwrap_or_else(|e| {
			error!("Failed to get current exe path: {e:#?}");
			args_os()
				.next()
				.expect("there is always the first arg")
				.into()
		})
		.parent()
		.and_then(|parent_path| {
			parent_path
				.join(path.as_ref())
				.canonicalize()
				.map_err(|e| error!("{e:#?}"))
				.ok()
		})
		.unwrap_or_else(|| path.as_ref().to_path_buf())
}
