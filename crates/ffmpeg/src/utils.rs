use crate::error::ThumbnailerError;
use std::ffi::CString;
use std::path::Path;

pub(crate) fn from_path(path: impl AsRef<Path>) -> Result<CString, ThumbnailerError> {
	let path = path.as_ref();
	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStrExt;
		CString::new(path.as_os_str().as_bytes())
			.map_err(|_| ThumbnailerError::PathConversion(path.to_path_buf()))
	}

	#[cfg(windows)]
	{
		use std::os::windows::ffi::OsStrExt;
		CString::from_vec_with_nul(
			path.as_os_str()
				.encode_wide()
				.chain(Some(0))
				.flat_map(|b| {
					let b = b.to_ne_bytes();
					b.first().copied().into_iter().chain(b.get(1).copied())
				})
				.collect::<Vec<u8>>(),
		)
		.map_err(|_| ThumbnailerError::PathConversion(path.to_path_buf()))
	}
}
