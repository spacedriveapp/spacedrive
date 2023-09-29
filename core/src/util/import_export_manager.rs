use crate::library::Library;
use crate::{DateTime, Utc};
use async_compression::tokio::bufread::GzipDecoder;
use async_compression::tokio::write::GzipEncoder;
use async_trait::async_trait;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use strum_macros::Display;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncWriteExt, BufReader};

#[derive(Debug, Deserialize, Clone)]
pub struct ImportExportOptions {
	pub output_path: PathBuf,
	pub kind: ExportKind,
	pub format: ExportFormat,
	pub compress: bool,
}

#[derive(Debug, Deserialize, Clone, Display)]
pub enum ExportFormat {
	JSON,
	CSV,
}

#[async_trait]
pub trait ImportExport<T>: Sync + Send {
	async fn export(&self, lib: &Library) -> Result<T, ImportExportError>;
	async fn import(&self, lib: &Library) -> Result<T, ImportExportError>;
}

#[derive(Debug, Deserialize, Clone, Display)]
pub enum ExportKind {
	Tags,
	TagWithAssociations,
	Location,
	LocationWithObjects,
	Album,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExportMetadata<T> {
	export_date: DateTime<Utc>,
	version: u8,
	kind: String,
	data: Option<T>,
}

#[derive(Error, Debug)]
pub enum ImportExportError {
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("compression error")]
	CompressionError,
	#[error("encryption error")]
	EncryptionError,
	#[error("item not found error")]
	NotFound,
	#[error("JSON conversion error: {0}")]
	JsonError(#[from] serde_json::Error),
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExportData<T, U = T> {
	Single(T),
	Multiple(Vec<U>),
}
pub struct ImportExportManager<T, U = T> {
	options: ImportExportOptions,
	data: ExportData<T, U>,
}

impl<T, U> ImportExportManager<T, U>
where
	T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
	U: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
	pub fn new(options: ImportExportOptions, data: ExportData<T, U>) -> Self {
		Self { options, data }
	}

	fn name(&self) -> String {
		format!(
			"SpacedriveExport-{}-{}",
			self.options.kind.to_string(),
			Utc::now().timestamp()
		)
	}

	pub fn get_data(&self) -> &ExportData<T, U> {
		&self.data
	}

	pub async fn save(&self) -> Result<(), ImportExportError> {
		let mut output_path = self.options.output_path.join(self.name());

		match &self.data {
			ExportData::Single(data) => {
				let raw_export_data = self.prepare_data(data).await?;

				if self.options.compress {
					output_path.set_extension("gz");

					let file = File::create(&output_path).await?;

					let mut encoder = GzipEncoder::new(file);

					encoder.write_all(raw_export_data.as_bytes()).await?;
					encoder
						.shutdown()
						.await
						.map_err(|_| ImportExportError::CompressionError)?;
				} else {
					output_path.set_extension(self.options.format.to_string().to_lowercase());
					let mut file = File::create(&output_path).await?;
					file.write_all(raw_export_data.as_bytes()).await?;
				}
			}
			ExportData::Multiple(items) => {
				output_path.set_extension("tar.gz");
				let tar_gz_file = std::fs::File::create(&output_path)?;

				let enc = GzEncoder::new(tar_gz_file, flate2::Compression::default());
				let mut tar = tar::Builder::new(enc);

				for (index, item) in items.iter().enumerate() {
					let raw_export_data = self.prepare_data(item).await?;
					let data = raw_export_data.as_bytes();

					let mut header = tar::Header::new_gnu();

					header.set_size(data.len() as u64);
					header.set_mode(0o755);

					// TODO: handle CSV case
					let file_name = format!("{}-{}.json", self.name(), index);

					tar.append_data(&mut header, file_name, data)?;
				}

				let enc = tar.into_inner()?;
				enc.finish()?;
			}
		};

		Ok(())
	}

	async fn prepare_data<V>(&self, data: &V) -> Result<String, ImportExportError>
	where
		V: Serialize,
	{
		let raw_export_data = match self.options.format {
			ExportFormat::JSON => serde_json::to_string(data)?,
			ExportFormat::CSV => {
				unimplemented!();
			}
		};

		Ok(raw_export_data)
	}

	pub async fn new_from_path(path: PathBuf) -> Result<Self, ImportExportError> {
		let file_extension = path
			.extension()
			.and_then(|s| s.to_str())
			.ok_or(ImportExportError::CompressionError)?;

		let content = if file_extension.ends_with("gz") {
			// Read and decompress the .gz or .tar.gz file
			let file = File::open(&path)
				.await
				.map_err(ImportExportError::IOError)?;

			let buf_reader = BufReader::new(file);
			let mut decoder = GzipDecoder::new(buf_reader);
			let mut buffer = Vec::new();

			decoder.read_to_end(&mut buffer).await?;
			buffer
		} else {
			// For non-gz files, read them directly
			tokio::fs::read(&path)
				.await
				.map_err(ImportExportError::IOError)?
		};

		// If it's a tar.gz file, extract JSON content from the tar archive
		let content_str = if file_extension == "tar.gz" {
			let mut archive = tar::Archive::new(&*content);
			let mut content_str = String::new();
			for file in archive
				.entries()
				.map_err(|_| ImportExportError::CompressionError)?
			{
				let mut file = file.map_err(|_| ImportExportError::CompressionError)?;
				file.read_to_string(&mut content_str)
					.map_err(ImportExportError::IOError)?;
			}
			content_str
		} else {
			String::from_utf8(content).map_err(|_| ImportExportError::CompressionError)?
		};

		let data: ExportData<T, U> = ExportData::Single(
			serde_json::from_str(&content_str).map_err(ImportExportError::JsonError)?,
		);

		let options = ImportExportOptions {
			output_path: path.parent().unwrap().to_path_buf(),
			kind: ExportKind::Tags,
			format: ExportFormat::JSON,
			compress: file_extension.ends_with("gz"),
		};

		Ok(Self { options, data })
	}
}
