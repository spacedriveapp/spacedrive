use crate::{location::LocationId, prisma::file_path, util::error::NonUtf8PathError};

use std::path::PathBuf;
use std::{borrow::Cow, path::Path};

use serde::{Deserialize, Serialize};

use super::{file_path_to_isolate, FilePathError};

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct IsolatedFilePathData<'a> {
	pub location_id: LocationId,
	pub materialized_path: Cow<'a, str>,
	pub is_dir: bool,
	pub name: Cow<'a, str>,
	pub extension: Cow<'a, str>,
}

impl IsolatedFilePathData<'static> {
	pub fn new(
		location_id: LocationId,
		location_path: impl AsRef<Path>,
		full_path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<Self, FilePathError> {
		let full_path = full_path.as_ref();
		let materialized_path =
			extract_normalized_materialized_path_str(location_id, location_path, full_path)?;

		let extension = if !is_dir {
			let extension = full_path
				.extension()
				.unwrap_or_default()
				.to_str()
				.unwrap_or_default();

			#[cfg(debug_assertions)]
			{
				// In dev mode, we lowercase the extension as we don't use the SQL migration,
				// and using prisma.schema directly we can't set `COLLATE NOCASE` in the
				// `extension` column at `file_path` table
				extension.to_lowercase()
			}
			#[cfg(not(debug_assertions))]
			{
				extension.to_string()
			}
		} else {
			String::new()
		};

		Ok(Self {
			materialized_path: Cow::Owned(materialized_path),
			is_dir,
			location_id,
			name: Cow::Owned(Self::prepare_name(full_path).to_string()),
			extension: Cow::Owned(extension),
		})
	}
}

impl<'a> IsolatedFilePathData<'a> {
	fn prepare_name(path: &Path) -> &str {
		// Not using `impl AsRef<Path>` here because it's an private method
		path.file_stem()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
	}

	pub fn parent(&self) -> IsolatedFilePathData<'static> {
		let parent_path = Path::new(self.materialized_path.as_ref())
			.parent()
			.unwrap_or_else(|| Path::new("/"));

		// putting a trailing '/' as `parent()` doesn't keep it
		let parent_path_str = format!(
			"{}/",
			parent_path
				.to_str()
				.expect("this expect is ok because this path was a valid UTF-8 String before")
		);

		IsolatedFilePathData {
			materialized_path: Cow::Owned(parent_path_str),
			is_dir: true,
			location_id: self.location_id,
			// NOTE: This way we don't use the same name for "/" `file_path`, that uses the location
			// name in the database, check later if this is a problem
			name: Cow::Owned(Self::prepare_name(parent_path).to_string()),
			extension: Cow::Owned(String::new()),
		}
	}

	pub fn from_db_data(
		location_id: LocationId,
		db_materialized_path: &'a str,
		db_is_dir: bool,
		db_name: &'a str,
		db_extension: &'a str,
	) -> Self {
		Self {
			location_id,
			materialized_path: Cow::Borrowed(db_materialized_path),
			is_dir: db_is_dir,
			name: Cow::Borrowed(db_name),
			extension: Cow::Borrowed(db_extension),
		}
	}

	pub fn from_relative_str(location_id: LocationId, relative_file_path_str: &'a str) -> Self {
		let is_dir = relative_file_path_str.ends_with('/');

		let (materialized_path, maybe_name, maybe_extension) =
			Self::separate_path_name_and_extension_from_str(relative_file_path_str);

		Self {
			location_id,
			materialized_path: Cow::Borrowed(materialized_path),
			is_dir,
			name: maybe_name.map(Cow::Borrowed).unwrap_or_default(),
			extension: maybe_extension.map(Cow::Borrowed).unwrap_or_default(),
		}
	}

	pub fn materialized_path_for_children(&self) -> Option<String> {
		self.is_dir
			.then(|| format!("{}/{}/", self.materialized_path, self.name))
	}

	pub fn to_relative_path_str(&self) -> String {
		match (self.is_dir, self.extension.as_ref()) {
			(true, _) => format!("{}/{}/", self.materialized_path, self.name),
			(false, "") => format!("{}/{}", self.materialized_path, self.name),
			(false, _) => format!(
				"{}/{}.{}",
				self.materialized_path, self.name, self.extension
			),
		}
	}

	pub fn to_path(&self) -> Box<Path> {
		PathBuf::from(match (self.is_dir, self.extension.as_ref()) {
			(false, extension) if extension != "" => {
				format!("{}/{}.{}", &self.materialized_path[1..], self.name, extension)
			}
			(_, _) => format!("{}/{}", &self.materialized_path[1..], self.name),
		})
		.into()
	}

	pub fn separate_path_name_and_extension_from_str(
		source: &'a str,
	) -> (
		&'a str,         // Materialized path
		Option<&'a str>, // Maybe a name
		Option<&'a str>, // Maybe an extension
	) {
		let is_dir = source.ends_with('/');
		let length = source.len();

		if length == 1 {
			// The case for the root path
			(source, None, None)
		} else if is_dir {
			let first_name_char_idx = source[..(length - 1)].rfind('/').unwrap_or(0) + 1;
			(
				&source[..(first_name_char_idx - 1)],
				Some(&source[first_name_char_idx..(length - 1)]),
				None,
			)
		} else {
			let first_name_char_idx = source.rfind('/').unwrap_or(0) + 1;
			let end_idx = first_name_char_idx - 1;
			if let Some(last_dot_relative_idx) = source[first_name_char_idx..].rfind('.') {
				let last_dot_idx = first_name_char_idx + last_dot_relative_idx;
				(
					&source[..end_idx],
					Some(&source[first_name_char_idx..last_dot_idx]),
					Some(&source[last_dot_idx + 1..]),
				)
			} else {
				(
					&source[..end_idx],
					Some(&source[first_name_char_idx..]),
					None,
				)
			}
		}
	}
}

impl From<&IsolatedFilePathData<'_>> for file_path::UniqueWhereParam {
	fn from(path: &IsolatedFilePathData<'_>) -> Self {
		Self::LocationIdMaterializedPathNameExtensionEquals(
			path.location_id,
			path.materialized_path.to_string(),
			path.name.to_string(),
			path.extension.to_string(),
		)
	}
}

impl From<&IsolatedFilePathData<'_>> for file_path::WhereParam {
	fn from(path: &IsolatedFilePathData<'_>) -> Self {
		file_path::UniqueWhereParam::from(path).into()
	}
}

impl From<file_path::Data> for IsolatedFilePathData<'static> {
	fn from(path: file_path::Data) -> Self {
		Self {
			location_id: path.location_id,
			materialized_path: Cow::Owned(path.materialized_path),
			is_dir: path.is_dir,
			name: Cow::Owned(path.name),
			extension: Cow::Owned(path.extension),
		}
	}
}

impl From<file_path_to_isolate::Data> for IsolatedFilePathData<'static> {
	fn from(path: file_path_to_isolate::Data) -> Self {
		Self {
			location_id: path.location_id,
			materialized_path: Cow::Owned(path.materialized_path),
			is_dir: path.is_dir,
			name: Cow::Owned(path.name),
			extension: Cow::Owned(path.extension),
		}
	}
}

/// This function separates a file path from a location path, and normalizes replacing '\' with '/'
/// to be consistent between Windows and Unix like systems
pub fn extract_normalized_materialized_path_str(
	location_id: LocationId,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<String, FilePathError> {
	let path = path.as_ref();

	path.strip_prefix(location_path)
		.map_err(|_| FilePathError::UnableToExtractMaterializedPath {
			location_id,
			path: path.into(),
		})?
		.parent()
		.map(|materialized_path| {
			materialized_path
				.to_str()
				.map(|materialized_path_str| {
					format!("/{}/", materialized_path_str.replace('\\', "/"))
				})
				.ok_or_else(|| NonUtf8PathError(path.into()))
		})
		.unwrap_or_else(|| Ok("/".to_string()))
		.map_err(Into::into)
}
