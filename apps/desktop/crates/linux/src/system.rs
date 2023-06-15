use std::{
	collections::{HashMap, HashSet},
	convert::TryFrom,
	ffi::OsString,
};

use mime::Mime;
use xdg_mime::SharedMimeInfo;

use crate::{DesktopEntry, Handler, HandlerType, Result};

#[derive(Debug, Default, Clone)]
pub struct SystemApps(pub HashMap<Mime, Vec<Handler>>);

impl SystemApps {
	pub fn get_handlers(&self, handler_type: HandlerType) -> Vec<Handler> {
		let mut handlers = match handler_type {
			HandlerType::Ext(ext) => {
				let mut handlers: HashSet<Handler> = HashSet::new();
				for mime in SharedMimeInfo::new().get_mime_types_from_file_name(ext.as_str()) {
					if let Some(mime_handlers) = self.0.get(&mime) {
						for handler in mime_handlers {
							handlers.insert(handler.clone());
						}
					}
				}
				handlers.into_iter().collect()
			}
			HandlerType::Mime(mime) => self.0.get(&mime).unwrap_or(&Vec::new()).clone(),
		};

		handlers.sort();

		handlers
	}

	pub fn get_handler(&self, handler_type: HandlerType) -> Option<Handler> {
		Some(self.get_handlers(handler_type).get(0)?.clone())
	}

	pub fn get_entries() -> Result<impl Iterator<Item = (OsString, DesktopEntry)>> {
		Ok(xdg::BaseDirectories::new()?
			.list_data_files_once("applications")
			.into_iter()
			.filter(|p| p.extension().and_then(|x| x.to_str()) == Some("desktop"))
			.filter_map(|p| Some((p.file_name()?.to_owned(), DesktopEntry::try_from(&p).ok()?))))
	}

	pub fn populate() -> Result<Self> {
		let mut map = HashMap::<Mime, Vec<Handler>>::with_capacity(50);

		Self::get_entries()?.for_each(|(_, entry)| {
			let (file_name, mimes) = (entry.file_name, entry.mimes);
			mimes.into_iter().for_each(|mime| {
				map.entry(mime)
					.or_default()
					.push(Handler::assume_valid(file_name.clone()));
			});
		});

		Ok(Self(map))
	}
}
