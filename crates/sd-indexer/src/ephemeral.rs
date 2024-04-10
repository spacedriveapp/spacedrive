use std::{
	future::ready,
	io::{self, ErrorKind},
	path::PathBuf,
};

use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt, TryFutureExt};
use opendal::{Operator, Scheme};
use sd_core_file_path_helper::path_is_hidden;
use sd_core_indexer_rules::{IndexerRule, RuleKind};
use sd_file_ext::{extensions::Extension, kind::ObjectKind};
use serde::Serialize;
use specta::Type;

use crate::stream::TaskStream;

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32, // TODO: Use `ObjectKind` instead
	// TODO: Use `kind` instead and drop this
	pub is_dir: bool,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub size_in_bytes_bytes: Vec<u8>,
	pub hidden: bool,
}

pub async fn ephemeral(
	opendal: Operator,
	rules: Vec<IndexerRule>,
	path: &str,
) -> opendal::Result<impl Stream<Item = io::Result<NonIndexedPathItem>>> {
	let is_fs = opendal.info().scheme() == Scheme::Fs;
	let base_path = PathBuf::from(opendal.info().root());
	let mut lister = opendal.lister(&path).await?;

	Ok(TaskStream::new(move |tx| async move {
		let rules = &*rules;
		while let Some(entry) = lister.next().await {
			let base_path = base_path.clone();
			let result = ready(entry)
				.map_err(|err| io::Error::new(ErrorKind::Other, format!("OpenDAL: {err:?}")))
				.and_then(|entry| async move {
					let path = base_path.join(entry.path());

					let extension = (!path.is_dir())
						.then(|| {
							path.extension()
								.and_then(|s| s.to_str().map(str::to_string))
								.unwrap_or_default()
						})
						.unwrap_or_default();

					// Only Windows supports normalised files without FS access.
					// For now we only do normalisation for local files.
					let (relative_path, name) = if is_fs {
						crate::path::normalize_path(&path).map_err(|err| {
							io::Error::new(
								ErrorKind::Other,
								format!("Error normalising path '{path:?}': {err:?}"),
							)
						})?
					} else {
						(
							path.file_stem()
								.and_then(|s| s.to_str().map(str::to_string))
								.ok_or_else(|| {
									io::Error::new(
										ErrorKind::Other,
										"error on file '{path:?}: non UTF-8",
									)
								})?
								.to_string(),
							path.to_str()
								.expect("non UTF-8 path - is unreachable")
								.to_string(),
						)
					};

					let kind = if entry.metadata().is_dir() {
						ObjectKind::Folder
					} else if is_fs {
						Extension::resolve_conflicting(&path, false)
							.await
							.map(Into::into)
							.unwrap_or(ObjectKind::Unknown)
					} else {
						// TODO: Determine kind of remote files - https://linear.app/spacedriveapp/issue/ENG-1718/fix-objectkind-of-remote-files
						ObjectKind::Unknown
					};

					let result = IndexerRule::apply_all(rules, &path).await.map_err(|err| {
						io::Error::new(
							ErrorKind::Other,
							format!("Error running indexer rules on file '{path:?}': {err:?}"),
						)
					})?;

					// No OS Protected and No Hidden rules, must always be from this kind, should panic otherwise
					if result[&RuleKind::RejectFilesByGlob]
						.iter()
						.any(|reject| !reject)
					{
						return Ok(None); // Skip this file
					};

					// TODO: OpenDAL last modified time - https://linear.app/spacedriveapp/issue/ENG-1717/fix-modified-time
					// TODO: OpenDAL hidden files - https://linear.app/spacedriveapp/issue/ENG-1720/fix-hidden-files
					let (hidden, date_created, date_modified, size) = if is_fs {
						let mut path = path
							.to_str()
							.expect("comes from string so this is impossible")
							.to_string();

						// OpenDAL will *always* end in a `/` for directories, we strip it here so we can give the path to Tokio.
						if path.ends_with("/") {
							path.pop();
						}

						let metadata = tokio::fs::metadata(&path).await.map_err(|err| {
							io::Error::new(
								ErrorKind::Other,
								format!("Error getting metadata for '{path:?}': {err:?}"),
							)
						})?;

						(
							path_is_hidden(&path, &metadata),
							metadata
								.created()
								.map_err(|err| {
									io::Error::new(
									ErrorKind::Other,
									format!("Error determining created time for '{path:?}': {err:?}"),
								)
								})?
								.into(),
							metadata
								.modified()
								.map_err(|err| {
									io::Error::new(
									ErrorKind::Other,
									format!("Error determining modified time for '{path:?}': {err:?}"),
								)
								})?
								.into(),
							metadata.len(),
						)
					} else {
						(false, Default::default(), Default::default(), 0)
					};

					// TODO: Fix this - https://linear.app/spacedriveapp/issue/ENG-1725/fix-last-modified
					let date_modified = date_modified;
					// entry.metadata().last_modified().ok_or_else(|| {
					// 	io::Error::new(
					// 		ErrorKind::Other,
					// 		format!("Error getting modified time for '{path:?}'"),
					// 	)
					// })?;

					// TODO: Fix this - https://linear.app/spacedriveapp/issue/ENG-1726/fix-file-size
					let size = size;

					Ok(Some(NonIndexedPathItem {
						path: relative_path,
						name,
						extension,
						kind: kind as i32,
						is_dir: kind == ObjectKind::Folder,
						date_created,
						date_modified,
						// TODO
						// entry
						// 	.metadata()
						// 	.content_length()
						size_in_bytes_bytes: size.to_be_bytes().to_vec(),
						hidden,
					}))
				})
				.await;

			if tx
				.send(match result {
					Ok(Some(item)) => Ok(item),
					Ok(None) => continue,
					Err(err) => Err(err),
				})
				.await
				.is_err()
			{
				// Stream has been dropped.
				continue;
			}
		}
	}))
}
