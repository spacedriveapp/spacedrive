use crate::{
	location::file_path_helper::{file_path_to_handle_custom_uri, IsolatedFilePathData},
	prisma::{file_path, location},
	util::{db::*, error::FileIOError},
	Node,
};

use std::{
	io,
	mem::take,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

#[cfg(windows)]
use std::cmp::min;

use http_range::HttpRange;
use httpz::{
	http::{response::Builder, Method, Response, StatusCode},
	Endpoint, GenericEndpoint, HttpEndpoint, Request,
};
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use prisma_client_rust::QueryError;
use thiserror::Error;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tracing::error;
use uuid::Uuid;

// This LRU cache allows us to avoid doing a DB lookup on every request.
// The main advantage of this LRU Cache is for video files. Video files are fetch in multiple chunks and the cache prevents a DB lookup on every chunk reducing the request time from 15-25ms to 1-10ms.
type MetadataCacheKey = (Uuid, file_path::id::Type);
type NameAndExtension = (PathBuf, String);
static FILE_METADATA_CACHE: Lazy<Cache<MetadataCacheKey, NameAndExtension>> =
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
		Some(&"thumbnail") => handle_thumbnail(&node, &path, &req).await,
		Some(&"file") => handle_file(&node, &path, &req).await,
		_ => Err(HandleCustomUriError::BadRequest("Invalid operation!")),
	}
}

async fn read_file(mut file: File, length: u64, start: Option<u64>) -> io::Result<Vec<u8>> {
	let mut buf = Vec::with_capacity(length as usize);
	if let Some(start) = start {
		file.seek(SeekFrom::Start(start)).await?;
		file.take(length).read_to_end(&mut buf).await?;
	} else {
		file.read_to_end(&mut buf).await?;
	}

	Ok(buf)
}

fn cors(
	method: &Method,
	builder: &mut Builder,
) -> Option<Result<Response<Vec<u8>>, httpz::http::Error>> {
	*builder = take(builder).header("Access-Control-Allow-Origin", "*");
	if method == Method::OPTIONS {
		Some(
			take(builder)
				.header("Access-Control-Allow-Methods", "GET, HEAD, POST, OPTIONS")
				.header("Access-Control-Allow-Headers", "*")
				.header("Access-Control-Max-Age", "86400")
				.status(StatusCode::OK)
				.body(vec![]),
		)
	} else {
		None
	}
}

async fn handle_thumbnail(
	node: &Node,
	path: &[&str],
	req: &Request,
) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let method = req.method();
	let mut builder = Response::builder();
	if let Some(response) = cors(method, &mut builder) {
		return Ok(response?);
	}

	if path.len() < 3 {
		return Err(HandleCustomUriError::BadRequest(
			"Invalid number of parameters!",
		));
	}

	let mut thumbnail_path = node.config.data_directory().join("thumbnails");
	// if we ever wish to support multiple levels of sharding, we need only supply more params here
	for path_part in &path[1..] {
		thumbnail_path = thumbnail_path.join(path_part);
	}
	let filename = thumbnail_path.with_extension("webp");

	let file = File::open(&filename).await.map_err(|err| {
		if err.kind() == io::ErrorKind::NotFound {
			HandleCustomUriError::NotFound("file")
		} else {
			FileIOError::from((&filename, err)).into()
		}
	})?;

	let content_length = file
		.metadata()
		.await
		.map_err(|e| FileIOError::from((&filename, e)))?
		.len();

	Ok(builder
		.header("Content-Type", "image/webp")
		.header("Content-Length", content_length)
		.status(StatusCode::OK)
		.body(if method == Method::HEAD {
			vec![]
		} else {
			read_file(file, content_length, None)
				.await
				.map_err(|e| FileIOError::from((&filename, e)))?
		})?)
}

async fn handle_file(
	node: &Node,
	path: &[&str],
	req: &Request,
) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
	let method = req.method();
	let mut builder = Response::builder();
	if let Some(response) = cors(method, &mut builder) {
		return Ok(response?);
	}

	let library_id = path
		.get(1)
		.and_then(|id| Uuid::from_str(id).ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing library_id!")
		})?;

	let location_id = path
		.get(2)
		.and_then(|id| id.parse::<location::id::Type>().ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing location_id!")
		})?;

	let file_path_id = path
		.get(3)
		.and_then(|id| id.parse::<file_path::id::Type>().ok())
		.ok_or_else(|| {
			HandleCustomUriError::BadRequest("Invalid number of parameters. Missing file_path_id!")
		})?;

	let lru_cache_key = (library_id, file_path_id);

	let (file_path_full_path, extension) =
		if let Some(entry) = FILE_METADATA_CACHE.get(&lru_cache_key) {
			entry
		} else {
			let library = node
				.libraries
				.get_library(&library_id)
				.await
				.ok_or_else(|| HandleCustomUriError::NotFound("library"))?;

			let file_path = library
				.db
				.file_path()
				.find_unique(file_path::id::equals(file_path_id))
				.select(file_path_to_handle_custom_uri::select())
				.exec()
				.await?
				.ok_or_else(|| HandleCustomUriError::NotFound("object"))?;

			let location = maybe_missing(&file_path.location, "file_path.location")?;
			let path = maybe_missing(&location.path, "file_path.location.path")?;

			let lru_entry = (
				Path::new(path).join(IsolatedFilePathData::try_from((location_id, &file_path))?),
				maybe_missing(file_path.extension, "extension")?,
			);

			FILE_METADATA_CACHE.insert(lru_cache_key, lru_entry.clone());

			lru_entry
		};

	let file = File::open(&file_path_full_path).await.map_err(|err| {
		if err.kind() == io::ErrorKind::NotFound {
			HandleCustomUriError::NotFound("file")
		} else {
			FileIOError::from((&file_path_full_path, err)).into()
		}
	})?;

	// TODO: This should be determined from magic bytes when the file is indexed and stored it in the DB on the file path
	// https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/MIME_types/Common_types
	let mime_type = match extension.as_str() {
		// AAC audio
		"aac" => "audio/aac",
		// Musical Instrument Digital Interface (MIDI)
		"mid" | "midi" => "audio/midi, audio/x-midi",
		// MP3 audio
		"mp3" => "audio/mpeg",
		// MP4 audio
		"m4a" => "audio/mp4",
		// OGG audio
		"oga" => "audio/ogg",
		// Opus audio
		"opus" => "audio/opus",
		// Waveform Audio Format
		"wav" => "audio/wav",
		// WEBM audio
		"weba" => "audio/webm",
		// AVI: Audio Video Interleave
		"avi" => "video/x-msvideo",
		// MP4 video
		"mp4" | "m4v" => "video/mp4",
		#[cfg(not(target_os = "macos"))]
		// FIX-ME: This media types break macOS video rendering
		// MPEG transport stream
		"ts" => "video/mp2t",
		#[cfg(not(target_os = "macos"))]
		// FIX-ME: This media types break macOS video rendering
		// MPEG Video
		"mpeg" => "video/mpeg",
		// OGG video
		"ogv" => "video/ogg",
		// WEBM video
		"webm" => "video/webm",
		// 3GPP audio/video container (TODO: audio/3gpp if it doesn't contain video)
		"3gp" => "video/3gpp",
		// 3GPP2 audio/video container (TODO: audio/3gpp2 if it doesn't contain video)
		"3g2" => "video/3gpp2",
		// Quicktime movies
		"mov" => "video/quicktime",
		// Windows OS/2 Bitmap Graphics
		"bmp" => "image/bmp",
		// Graphics Interchange Format (GIF)
		"gif" => "image/gif",
		// Icon format
		"ico" => "image/vnd.microsoft.icon",
		// JPEG images
		"jpeg" | "jpg" => "image/jpeg",
		// Portable Network Graphics
		"png" => "image/png",
		// Scalable Vector Graphics (SVG)
		"svg" => "image/svg+xml",
		// Tagged Image File Format (TIFF)
		"tif" | "tiff" => "image/tiff",
		// WEBP image
		"webp" => "image/webp",
		// PDF document
		"pdf" => "application/pdf",
		// HEIF/HEIC images
		"heif" | "heifs" => "image/heif,image/heif-sequence",
		"heic" | "heics" => "image/heic,image/heic-sequence",
		// AVIF images
		"avif" | "avci" | "avcs" => "image/avif",
		_ => {
			return Err(HandleCustomUriError::BadRequest(
				"TODO: This filetype is not supported because of the missing mime type!",
			));
		}
	};

	let mut content_lenght = file
		.metadata()
		.await
		.map_err(|e| FileIOError::from((&file_path_full_path, e)))?
		.len();

	// GET is the only method for which range handling is defined, according to the spec
	// https://httpwg.org/specs/rfc9110.html#field.range
	let range = if method == Method::GET {
		if let Some(range) = req.headers().get("range") {
			range
				.to_str()
				.ok()
				.and_then(|range| HttpRange::parse(range, content_lenght).ok())
				.ok_or_else(|| {
					HandleCustomUriError::RangeNotSatisfiable("Error decoding range header!")
				})
				.and_then(|range| {
					// Let's support only 1 range for now
					if range.len() > 1 {
						Err(HandleCustomUriError::RangeNotSatisfiable(
							"Multiple ranges are not supported!",
						))
					} else {
						Ok(range.first().cloned())
					}
				})?
		} else {
			None
		}
	} else {
		None
	};

	let mut status_code = 200;
	let buf = match range {
		Some(range) => {
			let file_size = content_lenght;
			content_lenght = range.length;

			// TODO: For some reason webkit2gtk doesn't like this at all.
			// It causes it to only stream random pieces of any given audio file.
			// TODO: This causes macOS to freeze streaming mp4
			#[cfg(windows)]
			// prevent max_length;
			// specially on webview2
			if mime_type != "application/pdf" && range.length > file_size / 3 {
				// max size sent (400kb / request)
				// as it's local file system we can afford to read more often
				content_lenght = min(file_size - range.start, 1024 * 400);
			}

			// last byte we are reading, the length of the range include the last byte
			// who should be skipped on the header
			let last_byte = range.start + content_lenght - 1;

			// if the webview sent a range header, we need to send a 206 in return
			status_code = 206;

			// macOS and Windows supports audio and video, linux only supports audio
			builder = builder
				.header("Connection", "Keep-Alive")
				.header("Accept-Ranges", "bytes")
				.header(
					"Content-Range",
					format!("bytes {}-{}/{}", range.start, last_byte, file_size),
				);

			// FIXME: Add ETag support (caching on the webview)

			read_file(file, content_lenght, Some(range.start))
				.await
				.map_err(|e| FileIOError::from((&file_path_full_path, e)))?
		}
		_ if method == Method::HEAD => {
			builder = builder.header("Accept-Ranges", "bytes");
			vec![]
		}
		_ => read_file(file, content_lenght, None)
			.await
			.map_err(|e| FileIOError::from((&file_path_full_path, e)))?,
	};

	Ok(builder
		.header("Content-type", mime_type)
		.header("Content-Length", content_lenght)
		.status(status_code)
		.body(buf)?)
}

pub fn create_custom_uri_endpoint(node: Arc<Node>) -> Endpoint<impl HttpEndpoint> {
	GenericEndpoint::new(
		"/*any",
		[Method::HEAD, Method::OPTIONS, Method::GET, Method::POST],
		move |req: Request| {
			let node = node.clone();
			async move { handler(node, req).await.unwrap_or_else(Into::into) }
		},
	)
}

#[derive(Error, Debug)]
pub enum HandleCustomUriError {
	#[error("HandleCustomUriError::Http - {0}")]
	Http(#[from] httpz::http::Error),
	#[error("HandleCustomUriError::FileIO - {0}")]
	FileIO(#[from] FileIOError),
	#[error("HandleCustomUriError::QueryError - {0}")]
	QueryError(#[from] QueryError),
	#[error("HandleCustomUriError::BadRequest - {0}")]
	BadRequest(&'static str),
	#[error("HandleCustomUriError::RangeNotSatisfiable - invalid range {0}")]
	RangeNotSatisfiable(&'static str),
	#[error("HandleCustomUriError::NotFound - resource '{0}'")]
	NotFound(&'static str),
	#[error("HandleCustomUriError::MissingField - '{0}'")]
	MissingField(#[from] MissingFieldError),
}

impl From<HandleCustomUriError> for Response<Vec<u8>> {
	fn from(value: HandleCustomUriError) -> Self {
		let builder = Response::builder().header("Content-Type", "text/plain");

		(match value {
			HandleCustomUriError::Http(err) => {
				error!("Error creating http request/response: {:#?}", err);
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::FileIO(err) => {
				error!("IO error: {:#?}", err);
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
			HandleCustomUriError::QueryError(err) => {
				error!("Query error: {:#?}", err);
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
			HandleCustomUriError::RangeNotSatisfiable(msg) => {
				error!("Invalid Range header in request: {}", msg);
				builder
					.status(StatusCode::RANGE_NOT_SATISFIABLE)
					.body(msg.as_bytes().to_vec())
			}
			HandleCustomUriError::NotFound(resource) => builder.status(StatusCode::NOT_FOUND).body(
				format!("Resource '{resource}' not found")
					.as_bytes()
					.to_vec(),
			),
			HandleCustomUriError::MissingField(id) => {
				error!("Location <id = {id}> has no path");
				builder
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(b"Internal Server Error".to_vec())
			}
		})
		// SAFETY: This unwrap is ok as we have an hardcoded the response builders.
		.expect("internal error building hardcoded HTTP error response")
	}
}
