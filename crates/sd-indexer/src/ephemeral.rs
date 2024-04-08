use std::{
	future::ready,
	io::{self, ErrorKind},
	path::PathBuf,
};

use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt, TryFutureExt};
use opendal::{Operator, Scheme};
use sd_file_ext::{extensions::Extension, kind::ObjectKind};
use sd_file_path_helper::path_is_hidden;
use serde::Serialize;
use specta::Type;

use crate::{
	rules::{IndexerRule, RuleKind},
	stream::TaskStream,
};

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32,
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
	let mut lister = opendal.lister(&path).await?;

	Ok(TaskStream::new(move |tx| async move {
		let rules = &*rules;
		while let Some(entry) = lister.next().await {
			let result = ready(entry)
				.map_err(|err| io::Error::new(ErrorKind::Other, format!("OpenDAL: {err:?}")))
				.and_then(|entry| async move {
					let path = PathBuf::from(entry.path());

					let extension = (!path.is_dir())
						.then(|| {
							path.extension()
								.and_then(|s| s.to_str().map(str::to_string))
								.unwrap_or_default()
						})
						.unwrap_or_default();

					// Only Windows supports normalised files without FS access.
					// For now we only do normalisation for local files.
					let (name, path) = if is_fs {
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
							entry.path().to_string(),
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
					let (hidden, date_created) = if is_fs {
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
						)
					} else {
						(false, Default::default())
					};

					let date_modified = entry.metadata().last_modified().ok_or_else(|| {
						io::Error::new(
							ErrorKind::Other,
							format!("Error getting modified time for '{path:?}'"),
						)
					})?;

					Ok(Some(NonIndexedPathItem {
						path: entry.path().to_string(),
						name,
						extension,
						kind: kind as i32,
						date_created,
						date_modified,
						size_in_bytes_bytes: entry
							.metadata()
							.content_length()
							.to_be_bytes()
							.to_vec(),
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
