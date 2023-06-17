use std::{
	collections::{BTreeSet, HashMap},
	convert::TryFrom,
	ffi::OsStr,
};

use mime::Mime;
use xdg_mime::SharedMimeInfo;

use crate::{DesktopEntry, Handler, HandlerType, Result};

#[derive(Debug, Default, Clone)]
pub struct SystemApps(pub HashMap<Mime, BTreeSet<Handler>>);

impl SystemApps {
	pub fn get_handlers(&self, handler_type: HandlerType) -> impl Iterator<Item = &Handler> {
		let mimes = match handler_type {
			HandlerType::Ext(ext) => {
				SharedMimeInfo::new().get_mime_types_from_file_name(ext.as_str())
			}
			HandlerType::Mime(mime) => vec![mime],
		};

		let mut handlers: BTreeSet<&Handler> = BTreeSet::new();
		for mime in mimes {
			if let Some(mime_handlers) = self.0.get(&mime) {
				handlers.extend(mime_handlers.iter());
			}
		}

		handlers.into_iter()
	}

	pub fn get_handler(&self, handler_type: HandlerType) -> Option<&Handler> {
		self.get_handlers(handler_type).next()
	}

	pub fn get_entries() -> Result<impl Iterator<Item = DesktopEntry>> {
		Ok(xdg::BaseDirectories::new()?
			.list_data_files_once("applications")
			.into_iter()
			.filter(|p| p.extension().map_or(false, |x| x == OsStr::new("desktop")))
			.filter_map(|p| DesktopEntry::try_from(&p).ok()))
	}

	pub fn populate() -> Result<Self> {
		let mut map = HashMap::<Mime, BTreeSet<Handler>>::with_capacity(50);

		Self::get_entries()?.for_each(
			|DesktopEntry {
			     mimes, file_name, ..
			 }| {
				mimes.into_iter().for_each(|mime| {
					map.entry(mime)
						.or_default()
						.insert(Handler::assume_valid(file_name.clone()));
				});
			},
		);

		Ok(Self(map))
	}
}
