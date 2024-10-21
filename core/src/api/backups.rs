use crate::{
	invalidate_query,
	library::{Library, LibraryManagerError},
	Node,
};

use sd_utils::error::FileIOError;

use std::{
	cmp,
	path::{Path, PathBuf},
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use futures::executor::block_on;
use futures_concurrency::future::TryJoin;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Serialize, Serializer};
use specta::Type;
use tar::Archive;
use tempfile::tempdir;
use thiserror::Error;
use tokio::{
	fs::{self, File},
	io::{
		self, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
		BufWriter,
	},
	spawn,
};
use tracing::{error, info};
use uuid::Uuid;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("getAll", {
			#[derive(Serialize, Type)]
			pub struct Backup {
				#[serde(flatten)]
				header: Header,
				path: PathBuf,
			}

			#[derive(Serialize, Type)]
			pub struct GetAll {
				backups: Vec<Backup>,
				directory: PathBuf,
			}

			async fn process_backups(path: impl AsRef<Path>) -> Result<Vec<Backup>, BackupError> {
				let path = path.as_ref();

				let mut read_dir = fs::read_dir(path).await.map_err(|e| {
					FileIOError::from((&path, e, "Failed to read backups directory"))
				})?;

				let mut backups = vec![];

				while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
					FileIOError::from((path, e, "Failed to read next entry to backup"))
				})? {
					let entry_path = entry.path();

					let metadata = entry.metadata().await.map_err(|e| {
						FileIOError::from((
							&entry_path,
							e,
							"Failed to read metadata from backup entry",
						))
					})?;

					if metadata.is_file() {
						backups.push(async move {
							let mut file = File::open(&entry_path).await.map_err(|e| {
								FileIOError::from((&entry_path, e, "Failed to open backup entry"))
							})?;

							Header::read(&mut file, &entry_path)
								.await
								.map(|header| Backup {
									header,
									path: entry_path,
								})
						});
					}
				}

				backups.try_join().await
			}

			R.query(|node, _: ()| async move {
				let directory = node.data_dir.join("backups");

				let backups = match fs::metadata(&directory).await {
					Ok(_) => process_backups(directory.clone()).await.map_err(|e| {
						rspc::Error::with_cause(
							ErrorCode::InternalServerError,
							"Failed to fetch backups".to_string(),
							e,
						)
					})?,
					Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
					Err(e) => {
						return Err(
							FileIOError::from((&directory, e, "Failed to fetch backups")).into(),
						)
					}
				};

				Ok(GetAll { backups, directory })
			})
		})
		.procedure("backup", {
			R.with2(library())
				.mutation(
					|(node, library), _: ()| async move { Ok(start_backup(node, library).await) },
				)
		})
		.procedure("restore", {
			R.mutation(|node, path: PathBuf| async move {
				start_restore(node, path).await;
				Ok(())
			})
		})
		.procedure("delete", {
			R.mutation(|node, path: PathBuf| async move {
				fs::remove_file(path)
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

	spawn(async move {
		match do_backup(bkp_id, &node, &library).await {
			Ok(path) => {
				info!(
					backup_id = %bkp_id,
					library_id = %library.id,
					path = %path.display(),
					"Backup created!;",
				);
				invalidate_query!(library, "backups.getAll");
			}
			Err(e) => {
				error!(
					backup_id = %bkp_id,
					library_id = %library.id,
					?e,
					"Error with backup for library;",
				);

				// TODO: Alert user something went wrong
			}
		}
	});

	bkp_id
}

#[derive(Error, Debug)]
enum BackupError {
	#[error("library manager error: {0}")]
	LibraryManager(#[from] LibraryManagerError),
	#[error("malformed header")]
	MalformedHeader,
	#[error("Library already exists, please remove it and try again!")]
	LibraryAlreadyExists,

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

async fn do_backup(id: Uuid, node: &Node, library: &Library) -> Result<PathBuf, BackupError> {
	let backups_dir = node.data_dir.join("backups");
	fs::create_dir_all(&backups_dir)
		.await
		.map_err(|e| FileIOError::from((&backups_dir, e)))?;

	let timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards")
		.as_millis();

	let bkp_path = backups_dir.join(format!("{id}.bkp"));
	let mut bkp_file = BufWriter::new(
		File::create(&bkp_path)
			.await
			.map_err(|e| FileIOError::from((&bkp_path, e, "Failed to create backup file")))?,
	);

	// Header. We do this so the file is self-sufficient.
	Header {
		id,
		timestamp,
		library_id: library.id,
		library_name: library.config().await.name.to_string(),
	}
	.write(&mut bkp_file)
	.await
	.map_err(|e| FileIOError::from((&bkp_path, e, "Failed to create backup file")))?;

	// Introducing this adapter here to bridge tokio stuff to std::io stuff
	struct WriterAdapter(BufWriter<File>);

	impl std::io::Write for WriterAdapter {
		fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
			block_on(self.0.write(buf))
		}

		fn flush(&mut self) -> io::Result<()> {
			block_on(self.0.flush())
		}
	}

	// Regular tar.gz encoded data
	let mut tar = tar::Builder::new(GzEncoder::new(
		WriterAdapter(bkp_file),
		Compression::default(),
	));

	let library_config_path = node
		.libraries
		.libraries_dir
		.join(format!("{}.sdlibrary", library.id));

	tar.append_file(
		"library.sdlibrary",
		&mut std::fs::File::open(&library_config_path).map_err(|e| {
			FileIOError::from((
				library_config_path,
				e,
				"Failed to open library config file to do a backup",
			))
		})?,
	)
	.map_err(|e| {
		FileIOError::from((
			&bkp_path,
			e,
			"Failed to append library config file to out backup tar.gz file",
		))
	})?;

	let library_db_path = node
		.libraries
		.libraries_dir
		.join(format!("{}.db", library.id));

	tar.append_file(
		"library.db",
		&mut std::fs::File::open(&library_db_path).map_err(|e| {
			FileIOError::from((
				library_db_path,
				e,
				"Failed to open library database file to do a backup",
			))
		})?,
	)
	.map_err(|e| {
		FileIOError::from((
			&bkp_path,
			e,
			"Failed to append library database file to out backup tar.gz file",
		))
	})?;

	Ok(bkp_path)
}

async fn start_restore(node: Arc<Node>, path: PathBuf) {
	match restore_backup(&node, &path).await {
		Ok(Header { id, library_id, .. }) => {
			info!(%id, %library_id, "Restored backup for library!");
		}
		Err(e) => {
			error!(path = %path.display(), ?e, "Error restoring backup;");

			// TODO: Alert user something went wrong
		}
	}
}

async fn restore_backup(node: &Arc<Node>, path: impl AsRef<Path>) -> Result<Header, BackupError> {
	let path = path.as_ref();

	let mut file = BufReader::new(fs::File::open(path).await.map_err(|e| {
		FileIOError::from((path, e, "Failed trying to open backup file to be restored"))
	})?);

	let header = Header::read(&mut file, path).await?;

	// TODO: Actually handle restoring into a library that exists. For now it's easier to error out.
	let None = node.libraries.get_library(&header.library_id).await else {
		return Err(BackupError::LibraryAlreadyExists);
	};

	let temp_dir = tempdir().map_err(|e| {
		FileIOError::from((
			"/tmp",
			e,
			"Failed to get a temporary directory to restore backup",
		))
	})?;

	// Introducing this adapter here to bridge tokio stuff to std::io stuff
	struct ReaderAdapter(BufReader<File>);

	impl std::io::Read for ReaderAdapter {
		fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
			block_on(self.0.read(buf))
		}
	}

	impl std::io::BufRead for ReaderAdapter {
		fn fill_buf(&mut self) -> io::Result<&[u8]> {
			block_on(self.0.fill_buf())
		}

		fn consume(&mut self, amt: usize) {
			self.0.consume(amt)
		}
	}

	let temp_dir_path = temp_dir.path();

	let mut archive = Archive::new(GzDecoder::new(ReaderAdapter(file)));
	archive.unpack(&temp_dir).map_err(|e| {
		FileIOError::from((temp_dir_path, e, "Failed to unpack backup compressed data"))
	})?;

	let library_config_path = temp_dir_path.join("library.sdlibrary");

	let library_config_restored_path = node
		.libraries
		.libraries_dir
		.join(format!("{}.sdlibrary", header.library_id));

	fs::copy(library_config_path, &library_config_restored_path)
		.await
		.map_err(|e| {
			FileIOError::from((
				&library_config_restored_path,
				e,
				"Failed to restore library config file from backup",
			))
		})?;

	let db_path = temp_dir_path.join("library.db");
	let db_restored_path = node
		.libraries
		.libraries_dir
		.join(format!("{}.db", header.library_id));

	fs::copy(db_path, &db_restored_path).await.map_err(|e| {
		FileIOError::from((
			&db_restored_path,
			e,
			"Failed to restore library database file from backup",
		))
	})?;

	node.libraries
		.load(
			header.library_id,
			db_restored_path,
			library_config_restored_path,
			None,
			None,
			true,
			node,
		)
		.await?;

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
	async fn write(&self, file: &mut (impl AsyncWrite + Unpin)) -> Result<(), io::Error> {
		// For future versioning we can bump `1` to `2` and match on it in the decoder.
		file.write_all(b"sdbkp1").await?;
		file.write_all(&self.id.to_bytes_le()).await?;
		file.write_all(&self.timestamp.to_le_bytes()).await?;
		file.write_all(&self.library_id.to_bytes_le()).await?;
		{
			let bytes = &self.library_name.as_bytes()
				[..cmp::min(u32::MAX as usize, self.library_name.len())];
			file.write_all(&(bytes.len() as u32).to_le_bytes()).await?;
			file.write_all(bytes).await?;
		}

		Ok(())
	}

	async fn read(
		file: &mut (impl AsyncRead + Unpin),
		path: impl AsRef<Path>,
	) -> Result<Self, BackupError> {
		let mut buf = vec![0u8; 6 + 16 + 16 + 16 + 4];
		let path = path.as_ref();
		file.read_exact(&mut buf)
			.await
			.map_err(|e| FileIOError::from((path, e)))?;

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
				file.read_exact(&mut name)
					.await
					.map_err(|e| FileIOError::from((path, e)))?;

				String::from_utf8(name).map_err(|_| BackupError::MalformedHeader)?
			},
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_backup_header() {
		let original = Header {
			id: Uuid::new_v4(),
			timestamp: 1234567890,
			library_id: Uuid::new_v4(),
			library_name: "Test Library".to_string(),
		};

		let mut buf = Vec::new();
		original.write(&mut buf).await.unwrap();

		let decoded = Header::read(&mut buf.as_slice(), "").await.unwrap();
		assert_eq!(original, decoded);
	}
}
