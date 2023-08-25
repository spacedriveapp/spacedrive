use std::{
	cmp,
	fs::{self, File},
	io::{self, BufReader, BufWriter, Read, Write},
	path::PathBuf,
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use futures::executor::block_on;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Serialize, Serializer};
use specta::Type;
use tar::Archive;
use tempfile::tempdir;
use thiserror::Error;
use tokio::task::spawn_blocking;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
	invalidate_query,
	library::{Library, LibraryManagerError},
	Node,
};

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("getAll", {
			#[derive(Serialize, Type)]
			pub struct Backup {
				#[serde(flatten)]
				header: Header,
				path: String,
			}

			#[derive(Serialize, Type)]
			pub struct GetAll {
				backups: Vec<Backup>,
				directory: String,
			}

			R.query(|node, _: ()| async move {
				let directory = node.data_dir.join("backups");

				Ok(GetAll {
					backups: if !directory.exists() {
						vec![]
					} else {
						spawn_blocking(move || {
							fs::read_dir(node.data_dir.join("backups"))
								.map(|dir| {
									dir.filter_map(|entry| {
										match entry.and_then(|e| Ok((e.metadata()?, e))) {
											Ok((metadata, entry)) if metadata.is_file() => {
												File::open(entry.path())
													.ok()
													.and_then(|mut file| {
														Header::read(&mut file).ok()
													})
													.map(|header| Backup {
														header,
														// TODO: Lossy strings are bad
														path: entry
															.path()
															.to_string_lossy()
															.to_string(),
													})
											}
											_ => None,
										}
									})
									.collect::<Vec<_>>()
								})
								.map_err(|e| {
									rspc::Error::with_cause(
										ErrorCode::InternalServerError,
										"Failed to fetch backups".to_string(),
										e,
									)
								})
						})
						.await
						.map_err(|e| {
							rspc::Error::with_cause(
								ErrorCode::InternalServerError,
								"Failed to fetch backups".to_string(),
								e,
							)
						})??
					},
					directory: directory.to_string_lossy().to_string(),
				})
			})
		})
		.procedure("backup", {
			R.with2(library())
				.mutation(|(node, library), _: ()| start_backup(node, library))
		})
		.procedure("restore", {
			R
				// TODO: Paths as strings is bad but here we want the flexibility of the frontend allowing any path
				.mutation(|node, path: String| start_restore(node, path.into()))
		})
		.procedure("delete", {
			R
				// TODO: Paths as strings is bad but here we want the flexibility of the frontend allowing any path
				.mutation(|node, path: String| async move {
					tokio::fs::remove_file(path)
						.await
						.map(|_| {
							invalidate_query!(node; node, "backups.getAll");
						})
						.map_err(|_| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Error deleting backup!".to_string(),
							)
						})
				})
		})
}

async fn start_backup(node: Arc<Node>, library: Arc<Library>) -> Uuid {
	let bkp_id = Uuid::new_v4();

	spawn_blocking(move || {
		match do_backup(bkp_id, &node, &library) {
			Ok(path) => {
				info!(
					"Backup '{bkp_id}' for library '{}' created at '{path:?}'!",
					library.id
				);
				invalidate_query!(library, "backups.getAll");
			}
			Err(e) => {
				error!(
					"Error with backup '{bkp_id}' for library '{}': {e:?}",
					library.id
				);

				// TODO: Alert user something went wrong
			}
		}
	});

	bkp_id
}

#[derive(Error, Debug)]
enum BackupError {
	#[error("io error: {0}")]
	Io(#[from] io::Error),
	#[error("library manager error: {0}")]
	LibraryManager(#[from] LibraryManagerError),
	#[error("malformed header")]
	MalformedHeader,
	#[error("Library already exists, please remove it and try again!")]
	LibraryAlreadyExists,
}

#[derive(Debug)]
pub struct MustRemoveLibraryErr;

// This is intended to be called in a `spawn_blocking` task.
// Async is pure overhead for an IO bound operation like this.
fn do_backup(id: Uuid, node: &Node, library: &Library) -> Result<PathBuf, BackupError> {
	let backups_dir = node.data_dir.join("backups");
	fs::create_dir_all(&backups_dir)?;

	let timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards")
		.as_millis();

	let bkp_path = backups_dir.join(format!("{id}.bkp"));
	let mut bkp_file = BufWriter::new(File::create(&bkp_path)?);

	// Header. We do this so the file is self-sufficient.
	Header {
		id,
		timestamp,
		library_id: library.id,
		library_name: library.config.name.to_string(),
	}
	.write(&mut bkp_file)?;

	// Regular tar.gz encoded data
	let mut tar = tar::Builder::new(GzEncoder::new(bkp_file, Compression::default()));

	tar.append_file(
		"library.sdlibrary",
		&mut File::open(
			node.libraries
				.libraries_dir
				.join(format!("{}.sdlibrary", library.id)),
		)?,
	)?;
	tar.append_file(
		"library.db",
		&mut File::open(
			node.libraries
				.libraries_dir
				.join(format!("{}.db", library.id)),
		)?,
	)?;

	Ok(bkp_path)
}

fn start_restore(node: Arc<Node>, path: PathBuf) {
	spawn_blocking(move || {
		match restore_backup(&node, path.clone()) {
			Ok(header) => {
				info!(
					"Restored to '{}' for library '{}'!",
					header.id, header.library_id
				);
			}
			Err(e) => {
				error!("Error restoring backup '{path:?}': {e:?}");

				// TODO: Alert user something went wrong
			}
		}
	});
}

fn restore_backup(node: &Arc<Node>, path: PathBuf) -> Result<Header, BackupError> {
	let mut file = BufReader::new(fs::File::open(path)?);
	let header = Header::read(&mut file)?;

	// TODO: Actually handle restoring into a library that exists. For now it's easier to error out.
	let None = block_on(node.libraries.get_library(&header.library_id)) else {
		return Err(BackupError::LibraryAlreadyExists);
	};

	let temp_dir = tempdir()?;

	let mut archive = Archive::new(GzDecoder::new(file));
	archive.unpack(&temp_dir)?;

	let library_path = temp_dir.path().join("library.sdlibrary");
	let db_path = temp_dir.path().join("library.db");

	fs::copy(
		library_path,
		node.libraries
			.libraries_dir
			.join(format!("{}.sdlibrary", header.library_id)),
	)?;
	fs::copy(
		db_path,
		node.libraries
			.libraries_dir
			.join(format!("{}.db", header.library_id)),
	)?;

	let config_path = node
		.libraries
		.libraries_dir
		.join(format!("{}.sdlibrary", header.library_id));
	let db_path = config_path.with_extension("db");
	block_on(
		node.libraries
			.load(header.library_id, &db_path, config_path, None, true, node),
	)?;

	Ok(header)
}

#[derive(Debug, PartialEq, Eq, Serialize, Type)]
struct Header {
	// Backup unique id
	id: Uuid,
	// Time since epoch the backup was created at
	#[specta(type = String)]
	#[serde(serialize_with = "as_string")]
	timestamp: u128,
	// Library id
	library_id: Uuid,
	// Library display name
	library_name: String,
}

fn as_string<T: ToString, S>(x: &T, s: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	s.serialize_str(&x.to_string())
}

impl Header {
	fn write(&self, file: &mut impl Write) -> Result<(), io::Error> {
		// For future versioning we can bump `1` to `2` and match on it in the decoder.
		file.write_all(b"sdbkp1")?;
		file.write_all(&self.id.to_bytes_le())?;
		file.write_all(&self.timestamp.to_le_bytes())?;
		file.write_all(&self.library_id.to_bytes_le())?;
		{
			let bytes = &self.library_name.as_bytes()
				[..cmp::min(u32::MAX as usize, self.library_name.len())];
			file.write_all(&(bytes.len() as u32).to_le_bytes())?;
			file.write_all(bytes)?;
		}

		Ok(())
	}

	fn read(file: &mut impl Read) -> Result<Self, BackupError> {
		let mut buf = vec![0u8; 6 + 16 + 16 + 16 + 4];
		file.read_exact(&mut buf)?;
		if &buf[..6] != b"sdbkp1" {
			return Err(BackupError::MalformedHeader);
		}

		Ok(Self {
			id: Uuid::from_bytes_le(
				buf[6..22]
					.try_into()
					.map_err(|_| BackupError::MalformedHeader)?,
			),
			timestamp: u128::from_le_bytes(
				buf[22..38]
					.try_into()
					.map_err(|_| BackupError::MalformedHeader)?,
			),
			library_id: Uuid::from_bytes_le(
				buf[38..54]
					.try_into()
					.map_err(|_| BackupError::MalformedHeader)?,
			),

			library_name: {
				let len = u32::from_le_bytes(
					buf[54..58]
						.try_into()
						.map_err(|_| BackupError::MalformedHeader)?,
				);

				let mut name = vec![0; len as usize];
				file.read_exact(&mut name)?;
				String::from_utf8(name).map_err(|_| BackupError::MalformedHeader)?
			},
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_backup_header() {
		let original = Header {
			id: Uuid::new_v4(),
			timestamp: 1234567890,
			library_id: Uuid::new_v4(),
			library_name: "Test Library".to_string(),
		};

		let mut buf = Vec::new();
		original.write(&mut buf).unwrap();

		let decoded = Header::read(&mut buf.as_slice()).unwrap();
		assert_eq!(original, decoded);
	}
}
