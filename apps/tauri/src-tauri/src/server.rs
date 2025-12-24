//! HTTP server for serving files and sidecars
//!
//! Tauri's custom URI protocols can't be async, so we use an Axum HTTP server
//! similar to the V1 implementation. The server is bound to localhost on a random
//! port and requires an auth token injected into the webview for security.

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

/// Validate a path segment to prevent directory traversal attacks.
///
/// Rejects segments containing `..`, path separators, or null bytes. These checks
/// prevent attackers from escaping the intended directory via URL-encoded sequences
/// like `%2E%2E` (which Axum decodes to `..` before we see it).
fn is_safe_path_segment(segment: &str) -> bool {
	// Reject empty segments
	if segment.is_empty() {
		return false;
	}

	// Reject segments containing path traversal or separator characters
	if segment.contains("..")
		|| segment.contains('/')
		|| segment.contains('\\')
		|| segment.contains('\0')
	{
		return false;
	}

	true
}

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
///
/// Supports both managed sidecars (content-addressed in library folder) and
/// ephemeral sidecars (entry-addressed in temp directory). Tries managed
/// first, falls back to ephemeral if not found.
async fn serve_sidecar(
	State(state): State<ServerState>,
	Path((library_id, content_uuid, kind, variant_and_ext)): Path<(String, String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
	// Try managed sidecar first (content-addressed)
	if let Ok(response) =
		serve_managed_sidecar(&state, &library_id, &content_uuid, &kind, &variant_and_ext).await
	{
		return Ok(response);
	}

	// Fall back to ephemeral sidecar (entry-addressed)
	serve_ephemeral_sidecar(&state, &library_id, &content_uuid, &kind, &variant_and_ext).await
}

/// Serve a managed sidecar (content-addressed in library folder)
async fn serve_managed_sidecar(
	state: &ServerState,
	library_id: &str,
	content_uuid: &str,
	kind: &str,
	variant_and_ext: &str,
) -> Result<Response<Body>, StatusCode> {
	// Security: validate all user-provided path segments to prevent directory traversal.
	// Axum URL-decodes segments, so `%2E%2E` becomes `..` before reaching this code.
	if !is_safe_path_segment(library_id)
		|| !is_safe_path_segment(content_uuid)
		|| !is_safe_path_segment(kind)
		|| !is_safe_path_segment(variant_and_ext)
	{
		error!(
			"Invalid path segment detected: library_id={:?}, content_uuid={:?}, kind={:?}, variant={:?}",
			library_id, content_uuid, kind, variant_and_ext
		);
		return Err(StatusCode::BAD_REQUEST);
	}

	// Find the actual library folder (might be named differently than the ID)
	let library_folder = find_library_folder(&state.data_dir, library_id).await?;

	// Actual path structure: sidecars/content/{first2}/{next2}/{uuid}/{kind}s/{variant}.{ext}
	// Example: sidecars/content/0c/c0/0cc0b48f-a475-53ec-a580-bc7d47b486a9/thumbs/detail@1x.webp
	// content_uuid is validated above, so indexing is safe (minimum 4 chars for UUID prefix)
	if content_uuid.len() < 4 {
		error!("Content UUID too short: {:?}", content_uuid);
		return Err(StatusCode::BAD_REQUEST);
	}
	let first_two = &content_uuid[0..2];
	let next_two = &content_uuid[2..4];

	// Special case: "transcript" stays singular (not "transcripts")
	let kind_dir = if kind == "transcript" {
		kind.to_string()
	} else {
		format!("{}s", kind) // "thumb" -> "thumbs"
	};

	let sidecar_path = library_folder
		.join("sidecars")
		.join("content")
		.join(first_two)
		.join(next_two)
		.join(content_uuid)
		.join(&kind_dir)
		.join(variant_and_ext);

	// Secondary defense: verify the constructed path is under the expected root.
	// This catches edge cases the segment validation might miss.
	let sidecars_root = state.data_dir.join("libraries");
	if !sidecar_path.starts_with(&sidecars_root) {
		error!(
			"Directory traversal attempt: {:?} not under {:?}",
			sidecar_path, sidecars_root
		);
		return Err(StatusCode::FORBIDDEN);
	}

	// Open the file
	let file = File::open(&sidecar_path).await.map_err(|e| {
		if e.kind() == io::ErrorKind::NotFound {
			StatusCode::NOT_FOUND
		} else {
			error!("Error opening managed sidecar {:?}: {}", sidecar_path, e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	})?;

	serve_file(file, variant_and_ext).await
}

/// Serve an ephemeral sidecar (entry-addressed in temp directory)
async fn serve_ephemeral_sidecar(
	_state: &ServerState,
	library_id: &str,
	entry_uuid: &str,
	kind: &str,
	variant_and_ext: &str,
) -> Result<Response<Body>, StatusCode> {
	// Security: validate all user-provided path segments to prevent directory traversal.
	// Axum URL-decodes segments, so `%2E%2E` becomes `..` before reaching this code.
	if !is_safe_path_segment(library_id)
		|| !is_safe_path_segment(entry_uuid)
		|| !is_safe_path_segment(kind)
		|| !is_safe_path_segment(variant_and_ext)
	{
		error!(
			"Invalid path segment in ephemeral: library_id={:?}, entry_uuid={:?}, kind={:?}, variant={:?}",
			library_id, entry_uuid, kind, variant_and_ext
		);
		return Err(StatusCode::BAD_REQUEST);
	}

	// Ephemeral path structure: /tmp/spacedrive-ephemeral-{library_id}/sidecars/entry/{entry_uuid}/{kind}s/{variant}.{ext}
	let temp_root = std::env::temp_dir()
		.join(format!("spacedrive-ephemeral-{}", library_id))
		.join("sidecars");

	let kind_dir = if kind == "transcript" {
		kind.to_string()
	} else {
		format!("{}s", kind)
	};

	let sidecar_path = temp_root
		.join("entry")
		.join(entry_uuid)
		.join(&kind_dir)
		.join(variant_and_ext);

	// Secondary defense: verify the constructed path is under the expected root.
	// This catches edge cases the segment validation might miss.
	if !sidecar_path.starts_with(&temp_root) {
		error!(
			"Directory traversal attempt in ephemeral: {:?} not under {:?}",
			sidecar_path, temp_root
		);
		return Err(StatusCode::FORBIDDEN);
	}

	// Open the file
	let file = File::open(&sidecar_path).await.map_err(|e| {
		if e.kind() == io::ErrorKind::NotFound {
			StatusCode::NOT_FOUND
		} else {
			error!("Error opening ephemeral sidecar {:?}: {}", sidecar_path, e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	})?;

	serve_file(file, variant_and_ext).await
}

/// Common file serving logic
async fn serve_file(file: File, variant_and_ext: &str) -> Result<Response<Body>, StatusCode> {
	let metadata = file.metadata().await.map_err(|e| {
		error!("Error reading file metadata: {}", e);
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
			"mp4" => Some("video/mp4"),
			"txt" => Some("text/plain"),
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

/// Create the HTTP router
fn create_router(data_dir: PathBuf) -> Router {
	let state = ServerState { data_dir };

	Router::new()
		.route(
			"/sidecar/:library_id/:content_uuid/:kind/*variant",
			get(serve_sidecar),
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
