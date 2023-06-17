#![cfg(target_os = "windows")]

use std::{
	ffi::{OsStr, OsString},
	os::windows::ffi::OsStrExt,
	path::Path,
};

use normpath::PathExt;
use windows::{
	core::{HSTRING, PCWSTR},
	Win32::{
		System::Com::{
			CoInitializeEx, CoUninitialize, IDataObject, COINIT_APARTMENTTHREADED,
			COINIT_DISABLE_OLE1DDE,
		},
		UI::Shell::{
			BHID_DataObject, IAssocHandler, IShellItem, SHAssocEnumHandlers,
			SHCreateItemFromParsingName, ASSOC_FILTER_RECOMMENDED,
		},
	},
};

pub use windows::core::{Error, Result};

// Based on: https://github.com/Byron/trash-rs/blob/841bc1388959ab3be4f05ad1a90b03aa6bcaea67/src/windows.rs#L212-L258
struct CoInitializer {}
impl CoInitializer {
	fn new() -> CoInitializer {
		let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
		if hr.is_err() {
			panic!("Call to CoInitializeEx failed. HRESULT: {:?}.", hr);
		}
		CoInitializer {}
	}
}

thread_local! {
	static CO_INITIALIZER: CoInitializer = {
		unsafe { libc::atexit(atexit_handler) };
		CoInitializer::new()
	};
}

extern "C" fn atexit_handler() {
	unsafe {
		CoUninitialize();
	}
}

fn ensure_com_initialized() {
	CO_INITIALIZER.with(|_| {});
}

// Use SHAssocEnumHandlers to get the list of apps associated with a file extension.
// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-shassocenumhandlers
pub fn list_apps_associated_with_ext(ext: &OsStr) -> Result<Vec<IAssocHandler>> {
	if ext.is_empty() {
		return Ok(Vec::new());
	}

	// SHAssocEnumHandlers requires the extension to be prefixed with a dot
	let ext = {
		// Get first charact from ext
		let ext_bytes = ext.encode_wide().collect::<Vec<_>>();
		if ext_bytes[0] != '.' as u16 {
			let mut prefixed_ext = OsString::from(".");
			prefixed_ext.push(ext);
			prefixed_ext
		} else {
			ext.to_os_string()
		}
	};

	let assoc_handlers =
		unsafe { SHAssocEnumHandlers(&HSTRING::from(ext), ASSOC_FILTER_RECOMMENDED) }?;

	let mut vec = Vec::new();
	loop {
		let mut rgelt = [None; 1];
		let mut pceltfetched = 0;
		unsafe { assoc_handlers.Next(&mut rgelt, Some(&mut pceltfetched)) }?;

		if pceltfetched == 0 {
			break;
		}

		if let [Some(handler)] = rgelt {
			vec.push(handler);
		}
	}

	Ok(vec)
}

pub fn open_file_path_with(path: &Path, url: &str) -> Result<()> {
	ensure_com_initialized();

	let ext = path.extension().ok_or(Error::OK)?;
	for handler in list_apps_associated_with_ext(ext)?.iter() {
		let name = unsafe { handler.GetName()?.to_string()? };
		if name == url {
			let path = path.normalize_virtually().map_err(|_| Error::OK)?;
			let wide_path = path
				.as_os_str()
				.encode_wide()
				.chain(std::iter::once(0))
				.collect::<Vec<_>>();
			let factory: IShellItem =
				unsafe { SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None) }?;
			let data: IDataObject = unsafe { factory.BindToHandler(None, &BHID_DataObject) }?;
			unsafe { handler.Invoke(&data) }?;

			return Ok(());
		}
	}

	Err(Error::OK)
}
