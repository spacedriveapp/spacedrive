#![cfg(target_os = "windows")]

use std::ffi::{OsStr, OsString};
use std::path::Path;

use normpath::PathExt;
use windows::core::HSTRING;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::UI::Shell::{
	BHID_DataObject, IAssocHandler, IShellItem, SHAssocEnumHandlers, SHCreateItemFromParsingName,
	ASSOC_FILTER_NONE,
};

pub use windows::core::{Error, Result};

// Use SHAssocEnumHandlers to get the list of apps associated with a file extension.
// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-shassocenumhandlers
pub fn list_apps_associated_with_ext(ext: &OsStr) -> Result<Vec<IAssocHandler>> {
	if ext.is_empty() {
		return Ok(Vec::new());
	}

	// SHAssocEnumHandlers requires the extension to be prefixed with a dot
	let ext = ext
		.to_str()
		.and_then(|str| str.chars().next())
		.and_then(|c| {
			if c == '.' {
				None
			} else {
				let mut prefixed_ext = OsString::new();
				prefixed_ext.push(".");
				prefixed_ext.push(ext);
				Some(prefixed_ext)
			}
		})
		.unwrap_or(ext.to_os_string());

	let assoc_handlers = unsafe { SHAssocEnumHandlers(&HSTRING::from(ext), ASSOC_FILTER_NONE) }?;

	let mut vec = Vec::new();
	loop {
		let mut rgelt = [None; 1];
		let mut pceltfetched = 0;
		unsafe { assoc_handlers.Next(&mut rgelt, Some(&mut pceltfetched)) }?;

		if pceltfetched == 0 {
			break;
		}

		for handler in rgelt.into_iter().flatten() {
			vec.push(handler);
		}
	}

	Ok(vec)
}

pub fn open_file_path_with(path: &Path, url: String) -> Result<()> {
	for handler in list_apps_associated_with_ext(path.extension().ok_or(Error::OK)?)?.iter() {
		let name = unsafe { handler.GetName() }
			.and_then(|name| -> Result<_> { unsafe { name.to_string() }.map_err(|_| Error::OK) })?;

		if name == url {
			let path = path.normalize_virtually().map_err(|_| Error::OK)?;
			let path_str = path.as_os_str();
			println!("Opening {:?} with {}", path_str, name);
			let factory: IShellItem =
				unsafe { SHCreateItemFromParsingName(&HSTRING::from(path_str), None) }?;
			println!("SHCreateItemFromParsingName");
			let data: IDataObject = unsafe { factory.BindToHandler(None, &BHID_DataObject) }?;
			println!("BindToHandler");
			unsafe { handler.Invoke(&data) }?;
			println!("Invoke");

			return Ok(());
		}
	}

	Err(Error::OK)
}
