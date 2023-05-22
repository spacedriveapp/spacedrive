use crate::{location::LocationId, prisma::file_path, util::error::NonUtf8PathError};

use std::{borrow::Cow, fmt, path::Path};

use serde::{Deserialize, Serialize};

use super::{
	file_path_for_file_identifier, file_path_for_object_validator, file_path_for_thumbnailer,
	file_path_to_full_path, file_path_to_handle_custom_uri, file_path_to_isolate,
	file_path_with_object, FilePathError,
};

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub struct IsolatedFilePathData<'a> {
	pub(in crate::location) location_id: LocationId,
	pub(in crate::location) materialized_path: Cow<'a, str>,
	pub(in crate::location) is_dir: bool,
	pub(in crate::location) name: Cow<'a, str>,
	pub(in crate::location) extension: Cow<'a, str>,
	pub(in crate::location) relative_path: Cow<'a, str>,
}

impl IsolatedFilePathData<'static> {
	pub fn new(
		location_id: LocationId,
		location_path: impl AsRef<Path>,
		full_path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<Self, FilePathError> {
		let full_path = full_path.as_ref();
		let location_path = location_path.as_ref();

		let extension = (!is_dir)
			.then(|| {
				full_path
					.extension()
					.unwrap_or_default()
					.to_str()
					.unwrap_or_default()
					// Coerce extension to lowercase to make it case-insensitive
					.to_lowercase()
			})
			.unwrap_or_default();

		Ok(Self {
			is_dir,
			location_id,
			materialized_path: Cow::Owned(extract_normalized_materialized_path_str(
				location_id,
				location_path,
				full_path,
			)?),
			name: Cow::Owned(
				(location_path != full_path)
					.then(|| Self::prepare_name(full_path).to_string())
					.unwrap_or_default(),
			),
			extension: Cow::Owned(extension),
			relative_path: Cow::Owned(extract_relative_path(
				location_id,
				location_path,
				full_path,
			)?),
		})
	}
}

impl<'a> IsolatedFilePathData<'a> {
	pub fn location_id(&self) -> LocationId {
		self.location_id
	}

	pub fn parent(&'a self) -> Self {
		let (parent_path_str, name, relative_path) = if self.materialized_path == "/" {
			("/", "", "")
		} else {
			let trailing_slash_idx = self.materialized_path.len() - 1;
			let last_slash_idx = self.materialized_path[..trailing_slash_idx]
				.rfind('/')
				.expect("malformed materialized path at `parent` method");

			(
				&self.materialized_path[..last_slash_idx + 1],
				&self.materialized_path[last_slash_idx + 1..trailing_slash_idx],
				&self.materialized_path[1..trailing_slash_idx],
			)
		};

		Self {
			is_dir: true,
			location_id: self.location_id,
			relative_path: Cow::Borrowed(relative_path),
			materialized_path: Cow::Borrowed(parent_path_str),
			name: Cow::Borrowed(name),
			extension: Cow::Borrowed(""),
		}
	}

	pub fn from_relative_str(location_id: LocationId, relative_file_path_str: &'a str) -> Self {
		let is_dir = relative_file_path_str.ends_with('/');

		let (materialized_path, maybe_name, maybe_extension) =
			Self::separate_path_name_and_extension_from_str(relative_file_path_str, is_dir);

		Self {
			location_id,
			materialized_path: Cow::Borrowed(materialized_path),
			is_dir,
			name: maybe_name.map(Cow::Borrowed).unwrap_or_default(),
			extension: maybe_extension.map(Cow::Borrowed).unwrap_or_default(),
			relative_path: Cow::Borrowed(relative_file_path_str),
		}
	}

	pub fn materialized_path_for_children(&self) -> Option<String> {
		if self.materialized_path == "/" && self.name.is_empty() && self.is_dir {
			// We're at the root file_path
			Some("/".to_string())
		} else {
			self.is_dir
				.then(|| format!("{}{}/", self.materialized_path, self.name))
		}
	}

	pub fn separate_path_name_and_extension_from_str(
		source: &'a str,
		is_dir: bool,
	) -> (
		&'a str,         // Materialized path
		Option<&'a str>, // Maybe a name
		Option<&'a str>, // Maybe an extension
	) {
		let length = source.len();

		if length == 1 {
			// The case for the root path
			(source, None, None)
		} else if is_dir {
			let last_char_idx = if source.ends_with('/') {
				length - 1
			} else {
				length
			};

			let first_name_char_idx = source[..last_char_idx].rfind('/').unwrap_or(0) + 1;
			(
				&source[..first_name_char_idx],
				Some(&source[first_name_char_idx..last_char_idx]),
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

	fn prepare_name(path: &Path) -> &str {
		// Not using `impl AsRef<Path>` here because it's an private method
		path.file_stem()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
	}

	fn from_db_data(
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
			relative_path: Cow::Owned(assemble_relative_path(
				db_materialized_path,
				db_name,
				db_extension,
				db_is_dir,
			)),
		}
	}
}

impl AsRef<Path> for IsolatedFilePathData<'_> {
	fn as_ref(&self) -> &Path {
		Path::new(self.relative_path.as_ref())
	}
}

impl From<IsolatedFilePathData<'static>> for file_path::UniqueWhereParam {
	fn from(path: IsolatedFilePathData<'static>) -> Self {
		Self::LocationIdMaterializedPathNameExtensionEquals(
			path.location_id,
			path.materialized_path.into_owned(),
			path.name.into_owned(),
			path.extension.into_owned(),
		)
	}
}

impl From<IsolatedFilePathData<'static>> for file_path::WhereParam {
	fn from(path: IsolatedFilePathData<'static>) -> Self {
		Self::And(vec![
			file_path::location_id::equals(path.location_id),
			file_path::materialized_path::equals(path.materialized_path.into_owned()),
			file_path::name::equals(path.name.into_owned()),
			file_path::extension::equals(path.extension.into_owned()),
		])
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
		Self::And(vec![
			file_path::location_id::equals(path.location_id),
			file_path::materialized_path::equals(path.materialized_path.to_string()),
			file_path::name::equals(path.name.to_string()),
			file_path::extension::equals(path.extension.to_string()),
		])
	}
}

impl fmt::Display for IsolatedFilePathData<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.relative_path)
	}
}

#[macro_use]
mod macros {
	macro_rules! impl_from_db {
		($($file_path_kind:ident),+ $(,)?) => {
			$(
				impl ::std::convert::From<$file_path_kind::Data> for $crate::
					location::
					file_path_helper::
					isolated_file_path_data::
					IsolatedFilePathData<'static>
				{
					fn from(path: $file_path_kind::Data) -> Self {
						Self {
							location_id: path.location_id,
							relative_path: ::std::borrow::Cow::Owned(
								$crate::
								location::
								file_path_helper::
								isolated_file_path_data::
								assemble_relative_path(
									&path.materialized_path,
									&path.name,
									&path.extension,
									path.is_dir,
								)
							),
							materialized_path: ::std::borrow::Cow::Owned(path.materialized_path),
							is_dir: path.is_dir,
							name: ::std::borrow::Cow::Owned(path.name),
							extension: ::std::borrow::Cow::Owned(path.extension),
						}
					}
				}

				impl<'a> ::std::convert::From<&'a $file_path_kind::Data> for $crate::
					location::
					file_path_helper::
					isolated_file_path_data::
					IsolatedFilePathData<'a>
				{
					fn from(path: &'a $file_path_kind::Data) -> Self {
						Self::from_db_data(
							path.location_id,
							&path.materialized_path,
							path.is_dir,
							&path.name,
							&path.extension
						)
					}
				}
			)+
		};
	}

	macro_rules! impl_from_db_without_location_id {
		($($file_path_kind:ident),+ $(,)?) => {
			$(
				impl ::std::convert::From<($crate::location::LocationId, $file_path_kind::Data)> for $crate::
					location::
					file_path_helper::
					isolated_file_path_data::
					IsolatedFilePathData<'static>
				{
					fn from((location_id, path): ($crate::location::LocationId, $file_path_kind::Data)) -> Self {
						Self {
							location_id,
							relative_path: Cow::Owned(
								$crate::
								location::
								file_path_helper::
								isolated_file_path_data::
								assemble_relative_path(
									&path.materialized_path,
									&path.name,
									&path.extension,
									path.is_dir,
								)
							),
							materialized_path: Cow::Owned(path.materialized_path),
							is_dir: path.is_dir,
							name: Cow::Owned(path.name),
							extension: Cow::Owned(path.extension),
						}
					}
				}

				impl<'a> ::std::convert::From<($crate::location::LocationId, &'a $file_path_kind::Data)> for $crate::
					location::
					file_path_helper::
					isolated_file_path_data::
					IsolatedFilePathData<'a>
				{
					fn from((location_id, path): ($crate::location::LocationId, &'a $file_path_kind::Data)) -> Self {
						Self::from_db_data(
							location_id,
							&path.materialized_path,
							path.is_dir,
							&path.name,
							&path.extension
						)
					}
				}
			)+
		};
	}
}

impl_from_db!(file_path, file_path_to_isolate, file_path_with_object);

impl_from_db_without_location_id!(
	file_path_for_file_identifier,
	file_path_to_full_path,
	file_path_for_thumbnailer,
	file_path_for_object_validator,
	file_path_to_handle_custom_uri
);

fn extract_relative_path(
	location_id: LocationId,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<String, FilePathError> {
	let path = path.as_ref();

	path.strip_prefix(location_path)
		.map_err(|_| FilePathError::UnableToExtractMaterializedPath {
			location_id,
			path: path.into(),
		})
		.and_then(|relative| {
			relative
				.to_str()
				.map(|relative_str| relative_str.replace('\\', "/"))
				.ok_or_else(|| NonUtf8PathError(path.into()).into())
		})
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
					if !materialized_path_str.is_empty() {
						format!("/{}/", materialized_path_str.replace('\\', "/"))
					} else {
						"/".to_string()
					}
				})
				.ok_or_else(|| NonUtf8PathError(path.into()))
		})
		.unwrap_or_else(|| Ok("/".to_string()))
		.map_err(Into::into)
}

fn assemble_relative_path(
	materialized_path: &str,
	name: &str,
	extension: &str,
	is_dir: bool,
) -> String {
	match (is_dir, extension) {
		(false, extension) if !extension.is_empty() => {
			format!("{}{}.{}", &materialized_path[1..], name, extension)
		}
		(_, _) => format!("{}{}", &materialized_path[1..], name),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn expected(
		materialized_path: &'static str,
		is_dir: bool,
		name: &'static str,
		extension: &'static str,
		relative_path: &'static str,
	) -> IsolatedFilePathData<'static> {
		IsolatedFilePathData {
			location_id: 1,
			materialized_path: materialized_path.into(),
			is_dir,
			name: name.into(),
			extension: extension.into(),
			relative_path: relative_path.into(),
		}
	}

	#[test]
	fn new_method() {
		let tester = |full_path, is_dir, expected, msg| {
			let actual =
				IsolatedFilePathData::new(1, "/spacedrive/location", full_path, is_dir).unwrap();
			assert_eq!(actual, expected, "{msg}");
		};

		tester(
			"/spacedrive/location",
			true,
			expected("/", true, "", "", ""),
			"the location root directory",
		);

		tester(
			"/spacedrive/location/file.txt",
			false,
			expected("/", false, "file", "txt", "file.txt"),
			"a file in the root directory",
		);

		tester(
			"/spacedrive/location/dir",
			true,
			expected("/", true, "dir", "", "dir"),
			"a directory in the root directory",
		);

		tester(
			"/spacedrive/location/dir/file.txt",
			false,
			expected("/dir/", false, "file", "txt", "dir/file.txt"),
			"a directory with a file inside",
		);

		tester(
			"/spacedrive/location/dir/dir2",
			true,
			expected("/dir/", true, "dir2", "", "dir/dir2"),
			"a directory in a directory",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3",
			true,
			expected("/dir/dir2/", true, "dir3", "", "dir/dir2/dir3"),
			"3 level of directories",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			false,
			expected(
				"/dir/dir2/dir3/",
				false,
				"file",
				"txt",
				"dir/dir2/dir3/file.txt",
			),
			"a file inside a third level directory",
		);
	}

	#[test]
	fn parent_method() {
		let tester = |full_path, is_dir, expected, msg| {
			let child =
				IsolatedFilePathData::new(1, "/spacedrive/location", full_path, is_dir).unwrap();

			let actual = child.parent();
			assert_eq!(actual, expected, "{msg}");
		};

		tester(
			"/spacedrive/location",
			true,
			expected("/", true, "", "", ""),
			"the location root directory",
		);

		tester(
			"/spacedrive/location/file.txt",
			false,
			expected("/", true, "", "", ""),
			"a file in the root directory",
		);

		tester(
			"/spacedrive/location/dir",
			true,
			expected("/", true, "", "", ""),
			"a directory in the root directory",
		);

		tester(
			"/spacedrive/location/dir/file.txt",
			false,
			expected("/", true, "dir", "", "dir"),
			"a directory with a file inside",
		);

		tester(
			"/spacedrive/location/dir/dir2",
			true,
			expected("/", true, "dir", "", "dir"),
			"a directory in a directory",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3",
			true,
			expected("/dir/", true, "dir2", "", "dir/dir2"),
			"3 level of directories",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			false,
			expected("/dir/dir2/", true, "dir3", "", "dir/dir2/dir3"),
			"a file inside a third level directory",
		);
	}

	#[test]
	fn extract_normalized_materialized_path() {
		let tester = |path, expected, msg| {
			let actual =
				extract_normalized_materialized_path_str(1, "/spacedrive/location", path).unwrap();
			assert_eq!(actual, expected, "{msg}");
		};

		tester("/spacedrive/location", "/", "the location root directory");
		tester(
			"/spacedrive/location/file.txt",
			"/",
			"a file in the root directory",
		);
		tester(
			"/spacedrive/location/dir",
			"/",
			"a directory in the root directory",
		);
		tester(
			"/spacedrive/location/dir/file.txt",
			"/dir/",
			"a directory with a file inside",
		);
		tester(
			"/spacedrive/location/dir/dir2",
			"/dir/",
			"a directory in a directory",
		);
		tester(
			"/spacedrive/location/dir/dir2/dir3",
			"/dir/dir2/",
			"3 level of directories",
		);
		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			"/dir/dir2/dir3/",
			"a file inside a third level directory",
		);
	}
}
