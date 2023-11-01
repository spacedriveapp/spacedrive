use crate::error::Error;
use std::ffi::CString;
use std::path::Path;

pub fn from_path(path: impl AsRef<Path>) -> Result<CString, Error> {
	let path = path.as_ref();
	let path_str = path.as_os_str();

	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStrExt;
		CString::new(path_str.as_bytes()).map_err(|_| Error::PathConversion(path.to_path_buf()))
	}
	#[cfg(not(unix))]
	{
		path_str
			.to_str()
			.and_then(|str| CString::new(str.as_bytes()).ok())
			.ok_or(Error::PathConversion(path.to_path_buf()))
	}
}
