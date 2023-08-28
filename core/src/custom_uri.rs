use crate::{
	location::file_path_helper::{file_path_to_handle_custom_uri, IsolatedFilePathData},
	prisma::{file_path, location},
	util::db::*,
	Node,
};

use std::{
	ffi::OsStr,
	io::SeekFrom,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
	time::UNIX_EPOCH,
};

use axum::{
	body::{self, Body, BoxBody, Full, StreamBody},
	extract::{self, State},
	http::{self, request, response, HeaderValue, Method, Request, Response, StatusCode},
	middleware::{self, Next},
	routing::get,
	Router,
};
use http_body::Limited;
use http_range::HttpRange;
use mini_moka::sync::Cache;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncSeekExt},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

type MetadataCacheKey = (Uuid, file_path::id::Type);
type NameAndExtension = (PathBuf, String);

#[derive(Clone)]
struct LocalState {
	node: Arc<Node>,

	// This LRU cache allows us to avoid doing a DB lookup on every request.
	// The main advantage of this LRU Cache is for video files. Video files are fetch in multiple chunks and the cache prevents a DB lookup on every chunk reducing the request time from 15-25ms to 1-10ms.
	// TODO: We should listen to events when deleting or moving a location and evict the cache accordingly.
	file_metadata_cache: Cache<MetadataCacheKey, NameAndExtension>,
}

// We are using Axum on all platforms because Tauri's custom URI protocols can't be async!
// TODO(@Oscar): Long-term hopefully this can be moved into rspc but streaming files is a hard thing for rspc to solve (Eg. how does batching work, dyn-safe handler, etc).
pub fn router(node: Arc<Node>) -> Router<()> {
	Router::new()
		.route(
			"/thumbnail/*path",
			get(
				|State(state): State<LocalState>,
				 extract::Path(path): extract::Path<String>,
				 request: Request<Body>| async move {
					let thumbnail_path = state.node.config.data_directory().join("thumbnails");
					let path = thumbnail_path.join(path);

					// Prevent directory traversal attacks (Eg. requesting `../../../etc/passwd`)
					if !path.starts_with(&thumbnail_path) {
						todo!(); // TODO: Error handling
					}

					// For now we only support `webp` thumbnails.
					if path.extension() != Some(OsStr::new("webp")) {
						todo!(); // TODO: Error handling
					}

					let file = File::open(&path).await.unwrap(); // TODO: Error handling

					serve_file(
						file,
						request.into_parts().0,
						Response::builder().header("Content-Type", "image/webp"),
					)
					.await
					.unwrap() // TODO: Error handling
				},
			),
		)
		.route(
			"/file/:lib_id/:loc_id/:path_id",
			get(
				|State(state): State<LocalState>,
				 extract::Path((lib_id, loc_id, path_id)): extract::Path<(
					String,
					String,
					String,
				)>,
				 request: Request<Body>| async move {
					let Ok(library_id) = Uuid::from_str(&lib_id) else {
						return Response::builder().status(400).body(body::boxed(Full::from("Library ID is not valid"))).unwrap(); // TODO: Error handling
					};
					let Ok(location_id) = loc_id.parse::<location::id::Type>() else {
						return Response::builder().status(400).body(body::boxed(Full::from("Location ID is not valid"))).unwrap(); // TODO: Error handling
					};
					let Ok(file_path_id) = path_id.parse::<file_path::id::Type>() else {
						return Response::builder().status(400).body(body::boxed(Full::from("Path ID is not valid"))).unwrap(); // TODO: Error handling
					};

					let lru_cache_key = (library_id, file_path_id);

					let (file_path_full_path, extension) = if let Some(entry) =
						state.file_metadata_cache.get(&lru_cache_key)
					{
						entry
					} else {
						let library = state.node.libraries.get_library(&library_id).await.unwrap(); // TODO: Error handling
																			// .ok_or_else(|| HandleCustomUriError::NotFound("library"))?;

						let file_path = library
							.db
							.file_path()
							.find_unique(file_path::id::equals(file_path_id))
							.select(file_path_to_handle_custom_uri::select())
							.exec()
							.await
							.unwrap()
							.unwrap(); // TODO: Error handling
		   // .ok_or_else(|| HandleCustomUriError::NotFound("object"))?;

						let location =
							maybe_missing(&file_path.location, "file_path.location").unwrap(); // TODO: Error handling
						let path =
							maybe_missing(&location.path, "file_path.location.path").unwrap(); // TODO: Error handling

						let lru_entry = (
							Path::new(path).join(
								IsolatedFilePathData::try_from((location_id, &file_path)).unwrap(), // TODO: Error handling
							),
							maybe_missing(file_path.extension, "extension").unwrap(), // TODO: Error handling
						);

						state
							.file_metadata_cache
							.insert(lru_cache_key, lru_entry.clone());

						lru_entry
					};

					let file = File::open(&file_path_full_path).await.unwrap(); // TODO: Error handling
					// .map_err(|err| {
					// 	if err.kind() == io::ErrorKind::NotFound {
					// 		HandleCustomUriError::NotFound("file")
					// 	} else {
					// 		FileIOError::from((&file_path_full_path, err)).into()
					// 	}
					// })?;

					serve_file(file, request.into_parts().0, Response::builder().header("Content-Type", plz_for_the_love_of_all_that_is_good_replace_this_with_the_db_instead_of_adding_variants_to_it(&extension)))
						.await
						.unwrap() // TODO: Error handling
				},
			),
		)
		.route_layer(middleware::from_fn(cors_middleware))
		.with_state(LocalState {
			node,
			file_metadata_cache: Cache::new(100),
		})
}

async fn cors_middleware<B>(req: Request<B>, next: Next<B>) -> Response<BoxBody> {
	if req.method() == Method::OPTIONS {
		return Response::builder()
			.header("Access-Control-Allow-Methods", "GET, HEAD, POST, OPTIONS")
			.header("Access-Control-Allow-Headers", "*")
			.header("Access-Control-Max-Age", "86400")
			.status(StatusCode::OK)
			.body(body::boxed(Full::from("")))
			.expect("Invalid static response!");
	}

	let mut response = next.run(req).await;

	response
		.headers_mut()
		.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));

	// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection
	response
		.headers_mut()
		.insert("Connection", HeaderValue::from_static("Keep-Alive"));

	response
		.headers_mut()
		.insert("Server", HeaderValue::from_static("Spacedrive"));

	response
}

/// Serve a Tokio file as a HTTP response.
///
/// This function takes care of:
///  - 304 Not Modified using ETag's
///  - Range requests for partial content
///
/// BE AWARE this function does not do any path traversal protection so that's up to the caller!
async fn serve_file(
	mut file: File,
	req: request::Parts,
	mut resp: response::Builder,
) -> http::Result<Response<BoxBody>> {
	// Handle `ETag` and `Content-Length` headers
	if let Ok(metadata) = file.metadata().await {
		// We only accept range queries if `files.metadata() == Ok(_)`
		// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Ranges
		resp = resp.header("Accept-Ranges", "bytes");

		if let Ok(time) = metadata.modified() {
			let etag_header = format!(
				r#""{}""#,
				// The ETag's can be any value so we just use the modified time to make it easy.
				time.duration_since(UNIX_EPOCH).unwrap().as_millis()
			);

			if let Some(etag) = req.headers.get("If-None-Match") {
				if etag.as_bytes() == etag_header.as_bytes() {
					return resp
						.status(StatusCode::NOT_MODIFIED)
						.body(body::boxed(Full::from("")));
				}
			}

			// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag
			resp = resp.header("etag", etag_header);
		}

		// https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests
		if req.method == Method::GET {
			if let Some(range) = req.headers.get("range") {
				// TODO: Error handling
				let ranges = HttpRange::parse(range.to_str().unwrap(), metadata.len()).unwrap();

				// TODO: Multipart requests are not support, yet
				if ranges.len() != 1 {
					todo!(); // TODO: Error handling
				}
				let range = ranges.first().expect("checked above");

				file.seek(SeekFrom::Start(range.start)).await.unwrap(); // TODO: Error handling

				// TODO: Serve using streaming body instead of loading the entire chunk. - Right now my impl is not working correctly
				let mut buf = Vec::with_capacity(range.length as usize);
				file.take(range.length).read_to_end(&mut buf).await.unwrap();

				return resp
					.status(StatusCode::PARTIAL_CONTENT)
					.header(
						"Content-Range",
						format!(
							"bytes {}-{}/{}",
							range.start,
							range.start + range.length - 1,
							metadata.len()
						),
					)
					.header("Content-Length", range.length.to_string())
					.body(body::boxed(Full::from(buf)));
				// TODO: Scope stream to range
				// .body(body::boxed(Limited::new(
				// 	StreamBody::new(ReaderStream::new(file)),
				// 	range.length.try_into().expect("integer overflow"),
				// )));
			}
		}
	}

	resp.body(body::boxed(StreamBody::new(ReaderStream::new(file))))
}

// TODO: This should be determined from magic bytes when the file is indexed and stored it in the DB on the file path
fn plz_for_the_love_of_all_that_is_good_replace_this_with_the_db_instead_of_adding_variants_to_it(
	ext: &str,
) -> &'static str {
	match ext {
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
		// TEXT document
		"txt" => "text/plain",
		_ => {
			todo!(); // TODO: Error handling
		}
	}
}
