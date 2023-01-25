use crate::prisma::file_path;
use crate::Node;
use http::{Request, Response, StatusCode};
use http_range::HttpRange;
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use prisma_client_rust::QueryError;
use std::{cmp::min, io, path::Path, path::PathBuf, str::FromStr};
use thiserror::Error;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tracing::{error, warn};
use uuid::Uuid;

// This LRU cache allows us to avoid doing a DB lookup on every request.
// The main advantage of this LRU Cache is for video files. Video files are fetch in multiple chunks and the cache prevents a DB lookup on every chunk reducing the request time from 15-25ms to 1-10ms.
type MetadataCacheKey = (Uuid, i32, i32);
static FILE_METADATA_CACHE: Lazy<Cache<MetadataCacheKey, (PathBuf, Option<String>)>> =
	Lazy::new(|| Cache::new(100));

// TODO: We should listen to events when deleting or moving a location and evict the cache accordingly.
// TODO: Probs use this cache in rspc queries too!

pub async fn handle_custom_uri(
	node: &Node,
	req: Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let path = req
		.uri()
		.path()
		.strip_prefix('/')
		.unwrap_or_else(|| req.uri().path())
		.split('/')
		.collect::<Vec<_>>();
	match path.first().copied() {
		Some("thumbnail") => {
			let file_cas_id = path
				.get(1)
				.ok_or_else(|| HandleCustomUriError::BadRequest("Invalid number of parameters!"))?;
			let filename = Path::new(&node.config.data_directory())
				.join("thumbnails")
				.join(file_cas_id)
				.with_extension("webp");

			let mut file = File::open(&filename)
				.await
				.map_err(|_| HandleCustomUriError::NotFound("file"))?;
			let mut buf = match file.metadata().await {
				Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
				Err(_) => Vec::new(),
			};

			file.read_to_end(&mut buf).await?;
			Ok(Response::builder()
				.header("Content-Type", "image/webp")
				.status(StatusCode::OK)
				.body(buf)?)
		}
		Some("file") => {
			let library_id = path
				.get(1)
				.and_then(|id| Uuid::from_str(id).ok())
				.ok_or_else(|| {
					HandleCustomUriError::BadRequest(
						"Invalid number of parameters. Missing library_id!",
					)
				})?;
			let location_id = path
				.get(2)
				.and_then(|id| id.parse::<i32>().ok())
				.ok_or_else(|| {
					HandleCustomUriError::BadRequest(
						"Invalid number of parameters. Missing location_id!",
					)
				})?;
			let file_path_id = path
				.get(3)
				.and_then(|id| id.parse::<i32>().ok())
				.ok_or_else(|| {
					HandleCustomUriError::BadRequest(
						"Invalid number of parameters. Missing file_path_id!",
					)
				})?;

			let lru_cache_key = (library_id, location_id, file_path_id);
			let (file_path_materialized_path, extension) =
				match FILE_METADATA_CACHE.get(&lru_cache_key) {
					Some(entry) => entry,
					None => {
						let library = node
							.library_manager
							.get_ctx(library_id)
							.await
							.ok_or_else(|| HandleCustomUriError::NotFound("library"))?;
						let file_path = library
							.db
							.file_path()
							.find_first(vec![
								file_path::id::equals(file_path_id),
								file_path::location_id::equals(location_id),
							])
							.with(file_path::location::fetch())
							.exec()
							.await?
							.ok_or_else(|| HandleCustomUriError::NotFound("object"))?;

						let lru_entry = (
							PathBuf::from(
								file_path
									.location
									.expect("unreachable location not fetched")
									.local_path
									.ok_or_else(|| {
										warn!(
											"Location '{}' doesn't have local path set",
											file_path.location_id
										);
										HandleCustomUriError::BadRequest(
											"Location doesn't have `local_path` set!",
										)
									})?,
							)
							.join(&file_path.materialized_path),
							file_path.extension,
						);
						FILE_METADATA_CACHE.insert(lru_cache_key, lru_entry.clone());
						lru_entry
					}
				};

			let mut file = File::open(file_path_materialized_path)
				.await
				.map_err(|_| HandleCustomUriError::NotFound("file"))?;
			let metadata = file.metadata().await?;

			// TODO: This should be determined from magic bytes when the file is indexer and stored it in the DB on the file path
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

			match is_video {
				true => {
					let mut response = Response::builder();
					let mut status_code = 200;

					// if the webview sent a range header, we need to send a 206 in return
					// Actually only macOS and Windows are supported. Linux will ALWAYS return empty headers.
					let buf = match req.headers().get("range") {
						Some(range) => {
							let mut buf = Vec::new();
							let file_size = metadata.len();
							let range = HttpRange::parse(
								range.to_str().map_err(|_| {
									HandleCustomUriError::BadRequest("Error passing range header!")
								})?,
								file_size,
							)
							.map_err(|_| {
								HandleCustomUriError::BadRequest("Error passing range!")
							})?;
							// let support only 1 range for now
							let first_range = range.first();
							if let Some(range) = first_range {
								let mut real_length = range.length;

								// prevent max_length;
								// specially on webview2
								if range.length > file_size / 3 {
									// max size sent (400ko / request)
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
										format!(
											"bytes {}-{}/{}",
											range.start, last_byte, file_size
										),
									);

								// FIXME: Add ETag support (caching on the webview)

								file.seek(SeekFrom::Start(range.start)).await?;
								file.take(real_length).read_to_end(&mut buf).await?;
							} else {
								file.read_to_end(&mut buf).await?;
							}

							buf
						}
						None => {
							// Linux is mega cringe and doesn't support streaming so we just load the whole file into memory and return it
							let mut buf = Vec::with_capacity(metadata.len() as usize);
							file.read_to_end(&mut buf).await?;
							buf
						}
					};

					Ok(response
						.header("Content-type", mime_type)
						.status(status_code)
						.body(buf)?)
				}
				false => {
					let mut buf = Vec::with_capacity(metadata.len() as usize);
					file.read_to_end(&mut buf).await?;
					Ok(Response::builder()
						.header("Content-Type", mime_type)
						.status(StatusCode::OK)
						.body(buf)?)
				}
			}
		}
		_ => Err(HandleCustomUriError::BadRequest("Invalid operation!")),
	}
}

#[derive(Error, Debug)]
pub enum HandleCustomUriError {
	#[error("error creating http request/response: {0}")]
	Http(#[from] http::Error),
	#[error("io error: {0}")]
	Io(#[from] io::Error),
	#[error("query error: {0}")]
	QueryError(#[from] QueryError),
	#[error("{0}")]
	BadRequest(&'static str),
	#[error("resource '{0}' not found")]
	NotFound(&'static str),
}

impl HandleCustomUriError {
	pub fn into_response(self) -> http::Result<Response<Vec<u8>>> {
		match self {
			HandleCustomUriError::Http(err) => {
				error!("Error creating http request/response: {}", err);
				Response::builder()
					.header("Content-Type", "text/plain")
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::Io(err) => {
				error!("IO error: {}", err);
				Response::builder()
					.header("Content-Type", "text/plain")
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::QueryError(err) => {
				error!("Query error: {}", err);
				Response::builder()
					.header("Content-Type", "text/plain")
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::BadRequest(msg) => {
				error!("Bad request: {}", msg);
				Response::builder()
					.header("Content-Type", "text/plain")
					.status(StatusCode::BAD_REQUEST)
					.body(msg.as_bytes().to_vec())
			}
			HandleCustomUriError::NotFound(resource) => Response::builder()
				.header("Content-Type", "text/plain")
				.status(StatusCode::NOT_FOUND)
				.body(
					format!("Resource '{}' not found", resource)
						.as_bytes()
						.to_vec(),
				),
		}
	}
}
