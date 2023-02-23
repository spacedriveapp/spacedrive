use crate::{prisma::file_path, Node};

use std::{
	cmp::min,
	io,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use http_range::HttpRange;
use httpz::{
	http::{Method, Response, StatusCode},
	Endpoint, GenericEndpoint, HttpEndpoint, Request,
};
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use prisma_client_rust::QueryError;
use thiserror::Error;
use tokio::{
	fs::{self, File},
	io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tracing::error;
use uuid::Uuid;

// This LRU cache allows us to avoid doing a DB lookup on every request.
// The main advantage of this LRU Cache is for video files. Video files are fetch in multiple chunks and the cache prevents a DB lookup on every chunk reducing the request time from 15-25ms to 1-10ms.
type MetadataCacheKey = (Uuid, i32, i32);
static FILE_METADATA_CACHE: Lazy<Cache<MetadataCacheKey, (PathBuf, Option<String>)>> =
	Lazy::new(|| Cache::new(100));

// TODO: We should listen to events when deleting or moving a location and evict the cache accordingly.
// TODO: Probs use this cache in rspc queries too!

async fn handler(node: Arc<Node>, req: Request) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let path = req
		.uri()
		.path()
		.strip_prefix('/')
		.unwrap_or_else(|| req.uri().path())
		.split('/')
		.collect::<Vec<_>>();

	match path.first() {
		Some(&"thumbnail") => handle_thumbnail(&node, &path).await,
		Some(&"file") => handle_file(&node, &path, &req).await,
		_ => Err(HandleCustomUriError::BadRequest("Invalid operation!")),
	}
}

async fn handle_thumbnail(
	node: &Node,
	path: &[&str],
) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let file_cas_id = path
		.get(1)
		.ok_or_else(|| HandleCustomUriError::BadRequest("Invalid number of parameters!"))?;
	let filename = node
		.config
		.data_directory()
		.join("thumbnails")
		.join(file_cas_id)
		.with_extension("webp");

	let buf = fs::read(&filename).await.map_err(|err| {
		if err.kind() == io::ErrorKind::NotFound {
			HandleCustomUriError::NotFound("file")
		} else {
			err.into()
		}
	})?;

	Ok(Response::builder()
		.header("Content-Type", "image/webp")
		.status(StatusCode::OK)
		.body(buf)?)
}

async fn handle_file(
	node: &Node,
	path: &[&str],
	req: &Request,
) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let library_id = path
		.get(1)
		.and_then(|id| Uuid::from_str(id).ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing library_id!")
		})?;

	let location_id = path
		.get(2)
		.and_then(|id| id.parse::<i32>().ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing location_id!")
		})?;

	let file_path_id = path
		.get(3)
		.and_then(|id| id.parse::<i32>().ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing file_path_id!")
		})?;

	let lru_cache_key = (library_id, location_id, file_path_id);

	let (file_path_materialized_path, extension) =
		if let Some(entry) = FILE_METADATA_CACHE.get(&lru_cache_key) {
			entry
		} else {
			let library = node
				.library_manager
				.get_ctx(library_id)
				.await
				.ok_or_else(|| HandleCustomUriError::NotFound("library"))?;
			let file_path = library
				.db
				.file_path()
				.find_unique(file_path::location_id_id(location_id, file_path_id))
				.include(file_path::include!({ location }))
				.exec()
				.await?
				.ok_or_else(|| HandleCustomUriError::NotFound("object"))?;

			let lru_entry = (
				Path::new(&file_path.location.path).join(&file_path.materialized_path),
				file_path.extension,
			);
			FILE_METADATA_CACHE.insert(lru_cache_key, lru_entry.clone());

			lru_entry
		};

	let mut file = File::open(file_path_materialized_path)
		.await
		.map_err(|err| {
			if err.kind() == io::ErrorKind::NotFound {
				HandleCustomUriError::NotFound("file")
			} else {
				err.into()
			}
		})?;

	let metadata = file.metadata().await?;

	// TODO: This should be determined from magic bytes when the file is indexed and stored it in the DB on the file path
	let (mime_type, is_video) = match extension.as_deref() {
		Some("mp4") => ("video/mp4", true),
		Some("webm") => ("video/webm", true),
		Some("mkv") => ("video/x-matroska", true),
		Some("avi") => ("video/x-msvideo", true),
		Some("mov") => ("video/quicktime", true),
		Some("png") => ("image/png", false),
		Some("jpg") => ("image/jpeg", false),
		Some("jpeg") => ("image/jpeg", false),
		Some("gif") => ("image/gif", false),
		Some("webp") => ("image/webp", false),
		Some("svg") => ("image/svg+xml", false),
		_ => {
			return Err(HandleCustomUriError::BadRequest(
				"TODO: This filetype is not supported because of the missing mime type!",
			));
		}
	};

	if is_video {
		let mut response = Response::builder();
		let mut status_code = 200;

		// if the webview sent a range header, we need to send a 206 in return
		let buf = if let Some(range) = req.headers().get("range") {
			let mut buf = Vec::new();
			let file_size = metadata.len();
			let range = HttpRange::parse(
				range
					.to_str()
					.map_err(|_| HandleCustomUriError::BadRequest("Error passing range header!"))?,
				file_size,
			)
			.map_err(|_| HandleCustomUriError::BadRequest("Error passing range!"))?;
			// let support only 1 range for now
			let first_range = range.first();
			if let Some(range) = first_range {
				let mut real_length = range.length;

				// prevent max_length;
				// specially on webview2
				if range.length > file_size / 3 {
					// max size sent (400kb / request)
					// as it's local file system we can afford to read more often
					real_length = min(file_size - range.start, 1024 * 400);
				}

				// last byte we are reading, the length of the range include the last byte
				// who should be skipped on the header
				let last_byte = range.start + real_length - 1;
				status_code = 206;

				// Only macOS and Windows are supported, if you set headers in linux they are ignored
				response = response
					.header("Connection", "Keep-Alive")
					.header("Accept-Ranges", "bytes")
					.header("Content-Length", real_length)
					.header(
						"Content-Range",
						format!("bytes {}-{}/{}", range.start, last_byte, file_size),
					);

				// FIXME: Add ETag support (caching on the webview)

				file.seek(SeekFrom::Start(range.start)).await?;
				file.take(real_length).read_to_end(&mut buf).await?;
			} else {
				file.read_to_end(&mut buf).await?;
			}

			buf
		} else {
			// Linux is mega cringe and doesn't support streaming so we just load the whole file into memory and return it
			let mut buf = Vec::with_capacity(metadata.len() as usize);
			file.read_to_end(&mut buf).await?;
			buf
		};

		Ok(response
			.header("Content-type", mime_type)
			.status(status_code)
			.body(buf)?)
	} else {
		let mut buf = Vec::with_capacity(metadata.len() as usize);
		file.read_to_end(&mut buf).await?;
		Ok(Response::builder()
			.header("Content-Type", mime_type)
			.status(StatusCode::OK)
			.body(buf)?)
	}
}

pub fn create_custom_uri_endpoint(node: Arc<Node>) -> Endpoint<impl HttpEndpoint> {
	GenericEndpoint::new("/*any", [Method::GET, Method::POST], move |req: Request| {
		let node = node.clone();
		async move { handler(node, req).await.unwrap_or_else(Into::into) }
	})
}

#[derive(Error, Debug)]
pub enum HandleCustomUriError {
	#[error("error creating http request/response: {0}")]
	Http(#[from] httpz::http::Error),
	#[error("io error: {0}")]
	Io(#[from] io::Error),
	#[error("query error: {0}")]
	QueryError(#[from] QueryError),
	#[error("{0}")]
	BadRequest(&'static str),
	#[error("resource '{0}' not found")]
	NotFound(&'static str),
}

impl From<HandleCustomUriError> for Response<Vec<u8>> {
	fn from(value: HandleCustomUriError) -> Self {
		let builder = Response::builder().header("Content-Type", "text/plain");

		(match value {
			HandleCustomUriError::Http(err) => {
				error!("Error creating http request/response: {}", err);
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::Io(err) => {
				error!("IO error: {}", err);
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::QueryError(err) => {
				error!("Query error: {}", err);
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::BadRequest(msg) => {
				error!("Bad request: {}", msg);
				builder
					.status(StatusCode::BAD_REQUEST)
					.body(msg.as_bytes().to_vec())
			}
			HandleCustomUriError::NotFound(resource) => builder.status(StatusCode::NOT_FOUND).body(
				format!("Resource '{resource}' not found")
					.as_bytes()
					.to_vec(),
			),
		})
		// SAFETY: This unwrap is ok as we have an hardcoded the response builders.
		.expect("internal error building hardcoded HTTP error response")
	}
}
