//! HTTP server for serving files, sidecars, and thumbnail hot-tier tiles.
//!
//! Tauri's custom URI protocols can't be async, so we use an Axum HTTP server
//! similar to the V1 implementation. The server is bound to localhost on a random
//! port and requires an auth token injected into the webview for security.
//!
//! Hot tiles come from the same `thumbs.pvcache` files the native client maps:
//! the daemon owns the writer, this process opens a read-only mapping, and the
//! tile is encoded to PNG on the way out because a browser cannot use raw
//! BGRA. Any number of readers are allowed, so this never contends with the
//! daemon or with another client.

use axum::{
	body::Body,
	extract::{Path, State},
	http::{header, HeaderValue, Request, Response, StatusCode},
	middleware::{self, Next},
	routing::get,
	Router,
};
use std::{net::Ipv4Addr, path::PathBuf};
use tokio::{fs::File, io, net::TcpListener};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct ServerState {
	/// Path to the Spacedrive data directory
	data_dir: PathBuf,
}

/// Find library folder by UUID (reads library.json files to match ID)
async fn find_library_folder(
	data_dir: &std::path::Path,
	library_id: &str,
) -> Result<PathBuf, StatusCode> {
	let libraries_dir = data_dir.join("libraries");

	// Read all .sdlibrary folders
	let mut entries = tokio::fs::read_dir(&libraries_dir)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	while let Some(entry) = entries
		.next_entry()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
	{
		let path = entry.path();
		if path.extension().and_then(|s| s.to_str()) == Some("sdlibrary") {
			// Try to read library.json
			let library_json_path = path.join("library.json");
			if let Ok(contents) = tokio::fs::read_to_string(&library_json_path).await {
				if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
					if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
						if id == library_id {
							return Ok(path);
						}
					}
				}
			}
		}
	}

	Err(StatusCode::NOT_FOUND)
}

/// Serve a sidecar file (e.g., thumbnail)
async fn serve_sidecar(
	State(state): State<ServerState>,
	Path((library_id, content_uuid, kind, variant_and_ext)): Path<(String, String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
	let content_uuid = content_uuid
		.parse::<uuid::Uuid>()
		.map_err(|_| StatusCode::BAD_REQUEST)?;

	let kind_dir = sd_sidecar_path::kind_directory(&kind).ok_or(StatusCode::BAD_REQUEST)?;

	// The variant filename is the only free-form path segment; keep it a
	// single segment to prevent directory traversal.
	if variant_and_ext.contains(['/', '\\']) || variant_and_ext.contains("..") {
		error!("Invalid sidecar variant segment: {:?}", variant_and_ext);
		return Err(StatusCode::FORBIDDEN);
	}

	// Find the actual library folder (might be named differently than the ID)
	let library_folder = find_library_folder(&state.data_dir, &library_id).await?;

	// The layout crate joins variant and extension itself, so split the
	// combined segment at the final dot.
	let (variant, ext) = variant_and_ext
		.rsplit_once('.')
		.ok_or(StatusCode::BAD_REQUEST)?;
	let sidecar_path = library_folder
		.join("sidecars")
		.join(sd_sidecar_path::relative_path(
			&content_uuid,
			kind_dir,
			variant,
			ext,
		));

	// Open the file
	let file = File::open(&sidecar_path).await.map_err(|e| {
		if e.kind() == io::ErrorKind::NotFound {
			error!("Sidecar file not found: {:?}", sidecar_path);
			StatusCode::NOT_FOUND
		} else {
			error!("Error opening sidecar {:?}: {}", sidecar_path, e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	})?;

	let metadata = file.metadata().await.map_err(|e| {
		error!("Error reading metadata for {:?}: {}", sidecar_path, e);
		StatusCode::INTERNAL_SERVER_ERROR
	})?;

	// Determine content type from extension
	let content_type = variant_and_ext
		.rsplit('.')
		.next()
		.and_then(|ext| match ext {
			"webp" => Some("image/webp"),
			"jpg" | "jpeg" => Some("image/jpeg"),
			"png" => Some("image/png"),
			_ => None,
		})
		.unwrap_or("application/octet-stream");

	// Build response with proper headers
	let content_length = metadata.len();
	let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));

	Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
		.header(header::CONTENT_LENGTH, content_length)
		.header(
			header::CACHE_CONTROL,
			HeaderValue::from_static("public, max-age=31536000, immutable"),
		)
		.header(
			header::ACCESS_CONTROL_ALLOW_ORIGIN,
			HeaderValue::from_static("*"),
		)
		.body(body)
		.map_err(|e| {
			error!("Error building response: {}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		})
}

/// CORS middleware to add headers to all responses (including errors)
async fn add_cors_headers(request: Request<Body>, next: Next) -> Response<Body> {
	let mut response = next.run(request).await;
	response.headers_mut().insert(
		header::ACCESS_CONTROL_ALLOW_ORIGIN,
		HeaderValue::from_static("*"),
	);
	response
}

/// Serve one hot-tier tile as a PNG.
///
/// The version in the path is the content version the caller expects. A slot
/// baked from different bytes is reported gone rather than served, so a client
/// never shows a tile for a file that has since changed; the caller re-reads
/// after the daemon announces the rebake.
async fn serve_hot_thumb(
	State(state): State<ServerState>,
	Path((source_id, record_uuid, version)): Path<(String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
	let source_id = Uuid::parse_str(&source_id).map_err(|_| StatusCode::BAD_REQUEST)?;
	let record_uuid = Uuid::parse_str(&record_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
	let version: u64 = version.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

	// Tiles are keyed by the volume index, not the source: two sources over
	// one drive share a cache. The id in the URL is the volume index id the
	// thumbs.request output named.
	let path = state
		.data_dir
		.join("volumes")
		.join(source_id.simple().to_string())
		.join("thumbs.pvcache");

	// Reading maps a file and copies a tile; both block, so it goes to the
	// blocking pool rather than stalling the server's runtime.
	let png = tokio::task::spawn_blocking(move || read_tile_png(&path, record_uuid, version))
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
		.ok_or(StatusCode::NOT_FOUND)?;

	Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, "image/png")
		// The version is in the URL, so a given URL's bytes never change.
		.header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
		.body(Body::from(png))
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Serve an on-demand video scrub sheet. Every path component is parsed as a
/// typed identity before it reaches the filesystem, and the content version is
/// part of the URL, so the response can be cached immutably.
async fn serve_hot_thumbstrip(
	State(state): State<ServerState>,
	Path((source_id, record_uuid, version)): Path<(String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
	let source_id = Uuid::parse_str(&source_id).map_err(|_| StatusCode::BAD_REQUEST)?;
	let record_uuid = Uuid::parse_str(&record_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
	let version: u64 = version.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
	let path = state
		.data_dir
		.join("volumes")
		.join(source_id.simple().to_string())
		.join("thumbstrips")
		.join(record_uuid.simple().to_string())
		.join(format!("{version}.png"));

	let file = tokio::fs::File::open(&path)
		.await
		.map_err(|_| StatusCode::NOT_FOUND)?;
	let content_length = file.metadata().await.ok().map(|metadata| metadata.len());
	let mut response = Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, "image/png")
		.header(header::CACHE_CONTROL, "public, max-age=31536000, immutable");
	if let Some(content_length) = content_length {
		response = response.header(header::CONTENT_LENGTH, content_length);
	}
	response
		.body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Copy a tile out of the cache and encode it as PNG. `None` when the slot is
/// empty, holds another version, or the file is not a cache.
///
/// The PNG carries the tile's own dimensions rather than the slot's: a slot is
/// a square envelope, and the frame inside it is the image's true aspect. A
/// grid draws that directly, and a square view crops it in CSS.
fn read_tile_png(path: &std::path::Path, uuid: Uuid, version: u64) -> Option<Vec<u8>> {
	let mut reader = sd_pvcache::PvcacheReader::open(path).ok()?;
	let mut bgra = vec![0u8; reader.tile_len()];
	let Ok(sd_pvcache::TileState::Fresh { frame }) = reader.get(uuid, version, &mut bgra) else {
		return None;
	};
	// Only the frame's rows were written; the rest of the envelope is not part
	// of the image.
	bgra.truncate(frame.len());
	// Tiles are stored BGRA8, which is what a GPU atlas wants; PNG wants RGBA.
	for pixel in bgra.chunks_exact_mut(4) {
		pixel.swap(0, 2);
	}
	let buffer = image::RgbaImage::from_raw(frame.content_width, frame.content_height, bgra)?;
	let mut png = std::io::Cursor::new(Vec::new());
	image::DynamicImage::ImageRgba8(buffer)
		.write_to(&mut png, image::ImageFormat::Png)
		.ok()?;
	Some(png.into_inner())
}

/// Create the HTTP router
fn create_router(data_dir: PathBuf) -> Router {
	let state = ServerState { data_dir };

	Router::new()
		.route(
			"/sidecar/:library_id/:content_uuid/:kind/*variant",
			get(serve_sidecar),
		)
		.route(
			"/hot-thumb/:source_id/:record_uuid/:version",
			get(serve_hot_thumb),
		)
		.route(
			"/hot-thumbstrip/:source_id/:record_uuid/:version",
			get(serve_hot_thumbstrip),
		)
		.layer(middleware::from_fn(add_cors_headers))
		.with_state(state)
}

/// Start the HTTP server on a random port
///
/// Returns the server address and a channel to trigger shutdown
pub async fn start_server(
	data_dir: PathBuf,
) -> Result<(String, tokio::sync::mpsc::Sender<()>), String> {
	// Bind to localhost on random port
	let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
		.await
		.map_err(|e| e.to_string())?;
	let addr = listener.local_addr().map_err(|e| e.to_string())?;
	let listen_url = format!("http://{}", addr);

	info!("Starting sidecar HTTP server on {}", listen_url);

	let app = create_router(data_dir);
	let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

	// Spawn server task
	tokio::spawn(async move {
		axum::serve(listener, app)
			.with_graceful_shutdown(async move {
				shutdown_rx.recv().await;
				info!("Shutting down sidecar HTTP server");
			})
			.await
			.expect("HTTP server error");
	});

	Ok((listen_url, shutdown_tx))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Tiles are stored BGRA and served RGBA, so a channel swap that went
	/// missing would show up as blue skies turning orange rather than as an
	/// error. The test pins the channel order end to end.
	#[test]
	fn a_stored_tile_encodes_to_png_with_its_channels_in_order() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = dir.path().join("thumbs.pvcache");
		let uuid = Uuid::from_u128(7);

		let mut writer = sd_pvcache::Pvcache::open_or_create(&path, 8, 8).expect("cache opens");
		let frame = sd_pvcache::Frame {
			content_width: 8,
			content_height: 8,
			source_width: 8,
			source_height: 8,
		};
		// Opaque red, written the way a baker writes it: B, G, R, A.
		let pixels: Vec<u8> = std::iter::repeat([0x00, 0x00, 0xFF, 0xFF])
			.take(8 * 8)
			.flatten()
			.collect();
		writer.write(uuid, 3, frame, &pixels).expect("tile writes");
		writer.flush().expect("flush");

		let png = read_tile_png(&path, uuid, 3).expect("tile reads back");
		let decoded = image::load_from_memory(&png).expect("valid png").to_rgba8();
		assert_eq!(decoded.dimensions(), (8, 8));
		assert_eq!(decoded.get_pixel(0, 0).0, [0xFF, 0x00, 0x00, 0xFF]);
	}

	#[test]
	fn a_tile_baked_from_other_bytes_is_not_served() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = dir.path().join("thumbs.pvcache");
		let uuid = Uuid::from_u128(9);

		let mut writer = sd_pvcache::Pvcache::open_or_create(&path, 8, 8).expect("cache opens");
		let frame = sd_pvcache::Frame {
			content_width: 8,
			content_height: 8,
			source_width: 8,
			source_height: 8,
		};
		writer
			.write(uuid, 1, frame, &vec![0u8; 8 * 8 * 4])
			.expect("tile writes");
		writer.flush().expect("flush");

		assert!(read_tile_png(&path, uuid, 2).is_none(), "stale version");
		assert!(
			read_tile_png(&path, Uuid::from_u128(10), 1).is_none(),
			"absent"
		);
	}

	/// An aspect-fit tile occupies part of its slot, so the PNG has to be built
	/// from the frame's dimensions. Taking the envelope's would read padding
	/// that was never written and skew every row after the first.
	#[test]
	fn an_aspect_tile_is_served_at_its_own_dimensions() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = dir.path().join("thumbs.pvcache");
		let uuid = Uuid::from_u128(11);

		let mut writer = sd_pvcache::Pvcache::open_or_create(&path, 16, 16).expect("cache opens");
		let frame = sd_pvcache::Frame {
			content_width: 16,
			content_height: 6,
			source_width: 4032,
			source_height: 1512,
		};
		let pixels: Vec<u8> = std::iter::repeat([0x00, 0xFF, 0x00, 0xFF])
			.take(16 * 6)
			.flatten()
			.collect();
		writer.write(uuid, 1, frame, &pixels).expect("tile writes");
		writer.flush().expect("flush");

		let png = read_tile_png(&path, uuid, 1).expect("tile reads back");
		let decoded = image::load_from_memory(&png).expect("valid png").to_rgba8();
		assert_eq!(decoded.dimensions(), (16, 6));
		assert_eq!(decoded.get_pixel(15, 5).0, [0x00, 0xFF, 0x00, 0xFF]);
	}
}
