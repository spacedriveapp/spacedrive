use axum::{
	body::Body,
	extract::{FromRequestParts, Request, State},
	http::{header, StatusCode, Uri},
	middleware::{self, Next},
	response::{
		sse::{Event as SseEvent, KeepAlive, Sse},
		IntoResponse, Response,
	},
	routing::{get, post},
	Json, Router,
};
use axum_extra::{headers::authorization::Basic, headers::Authorization, TypedHeader};
use clap::Parser;
use futures::stream::{Stream, StreamExt};
use rust_embed::Embed;
use secstr::SecStr;
use std::{
	collections::HashMap, convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc,
	time::Duration,
};
use tokio::{
	io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
	net::TcpStream,
	signal,
	sync::{mpsc, RwLock},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

/// Embedded web UI assets, built from `apps/web` via `bun run build`.
/// In debug builds, files are read from disk at request time, so editing
/// `apps/web/dist/` after a rebuild of the frontend is picked up live.
/// In release builds, contents are baked into the binary.
/// Source maps stay on disk for debugging and are excluded from the embed, since
/// they are several times the size of the bundles they describe.
#[derive(Embed)]
#[folder = "../web/dist/"]
#[exclude = "*.map"]
struct WebAssets;

#[derive(Clone)]
struct AppState {
	auth: HashMap<String, SecStr>,
	socket_addr: String,
	data_dir: PathBuf,
}

/// Basic auth middleware
async fn basic_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
	let request = if !state.auth.is_empty() {
		let (mut parts, body) = request.into_parts();

		let Ok(TypedHeader(Authorization(hdr))) =
			TypedHeader::<Authorization<Basic>>::from_request_parts(&mut parts, &()).await
		else {
			return Response::builder()
				.status(401)
				.header("WWW-Authenticate", "Basic realm=\"Spacedrive\"")
				.body("Unauthorized".into_response().into_body())
				.expect("hardcoded response will be valid");
		};
		let request = Request::from_parts(parts, body);

		if state
			.auth
			.get(hdr.username())
			.map(|pass| *pass == SecStr::from(hdr.password()))
			!= Some(true)
		{
			return Response::builder()
				.status(401)
				.header("WWW-Authenticate", "Basic realm=\"Spacedrive\"")
				.body("Unauthorized".into_response().into_body())
				.expect("hardcoded response will be valid");
		}

		request
	} else {
		request
	};

	next.run(request).await
}

/// Health check endpoint
async fn health() -> &'static str {
	"OK"
}

/// Serve the embedded web UI. Looks up the requested path in `WebAssets`;
/// if not found, falls back to `index.html` so client-side routing in the
/// SPA continues to work for deep links like `/explorer/foo/bar`.
async fn serve_web(uri: Uri) -> Response {
	let path = uri.path().trim_start_matches('/');
	let lookup = if path.is_empty() { "index.html" } else { path };

	if let Some(asset) = WebAssets::get(lookup) {
		let mime = mime_guess::from_path(lookup).first_or_octet_stream();
		return Response::builder()
			.header(header::CONTENT_TYPE, mime.as_ref())
			.body(Body::from(asset.data.into_owned()))
			.expect("static asset response is well-formed");
	}

	if let Some(index) = WebAssets::get("index.html") {
		return Response::builder()
			.header(header::CONTENT_TYPE, "text/html; charset=utf-8")
			.body(Body::from(index.data.into_owned()))
			.expect("index.html response is well-formed");
	}

	// Web bundle is missing entirely — sd-server was built without `apps/web/dist`.
	Response::builder()
		.status(StatusCode::NOT_FOUND)
		.header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
		.body(Body::from(
			"Spacedrive web UI is not bundled in this build. \
			 Run `bun run build` in `apps/web/` and rebuild sd-server.",
		))
		.expect("missing-bundle response is well-formed")
}

/// Locate a library folder by its UUID. Library directory names are
/// user-chosen, so each library.json is read to match the id.
async fn find_library_folder(data_dir: &std::path::Path, library_id: &str) -> Option<PathBuf> {
	let mut entries = tokio::fs::read_dir(data_dir.join("libraries")).await.ok()?;
	while let Ok(Some(entry)) = entries.next_entry().await {
		let path = entry.path();
		if path.extension().and_then(|s| s.to_str()) != Some("sdlibrary") {
			continue;
		}
		let Ok(contents) = tokio::fs::read_to_string(path.join("library.json")).await else {
			continue;
		};
		let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) else {
			continue;
		};
		if json.get("id").and_then(|v| v.as_str()) == Some(library_id) {
			return Some(path);
		}
	}
	None
}

fn plain_status(status: StatusCode, message: &'static str) -> Response {
	Response::builder()
		.status(status)
		.header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
		.body(Body::from(message))
		.expect("static status response is well-formed")
}

/// Serve a sidecar file (thumbnail, thumbstrip, proxy, …) from the library's
/// sidecars tree — the HTTP twin of the desktop app's local sidecar server,
/// built on sd-core's canonical path scheme.
async fn serve_sidecar(
	State(state): State<AppState>,
	axum::extract::Path((library_id, content_uuid, kind, variant_and_ext)): axum::extract::Path<(
		String,
		String,
		String,
		String,
	)>,
) -> Response {
	let Ok(content_uuid) = content_uuid.parse::<uuid::Uuid>() else {
		return plain_status(StatusCode::BAD_REQUEST, "invalid content uuid");
	};
	let Some(kind_dir) = sd_sidecar_path::kind_directory(&kind) else {
		return plain_status(StatusCode::BAD_REQUEST, "invalid sidecar kind");
	};
	let Some((variant, ext)) = variant_and_ext.rsplit_once('.') else {
		return plain_status(StatusCode::BAD_REQUEST, "variant must carry an extension");
	};
	// The variant is the only free-form path segment; keep it a single segment.
	if variant.contains(['/', '\\']) || variant.contains("..") {
		return plain_status(StatusCode::BAD_REQUEST, "invalid variant");
	}
	let Some(library_folder) = find_library_folder(&state.data_dir, &library_id).await else {
		return plain_status(StatusCode::NOT_FOUND, "unknown library");
	};

	let path = library_folder.join("sidecars").join(
		sd_sidecar_path::relative_path(&content_uuid, kind_dir, variant, ext),
	);

	let Ok(file) = tokio::fs::File::open(&path).await else {
		return plain_status(StatusCode::NOT_FOUND, "sidecar not found");
	};
	let content_length = file.metadata().await.ok().map(|m| m.len());

	let content_type = match ext {
		"webp" => "image/webp",
		"jpg" | "jpeg" => "image/jpeg",
		"png" => "image/png",
		"mp4" => "video/mp4",
		"json" => "application/json",
		"txt" => "text/plain; charset=utf-8",
		_ => "application/octet-stream",
	};

	// Sidecar paths are content-addressed (uuid + variant), so clients may
	// cache indefinitely; regenerated variants land at the same path only
	// when the content itself is unchanged.
	let mut builder = Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, content_type)
		.header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
		.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
	if let Some(len) = content_length {
		builder = builder.header(header::CONTENT_LENGTH, len);
	}
	builder
		.body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
		.expect("sidecar response is well-formed")
}

/// Bridge the daemon's event stream to a browser SSE connection.
///
/// Opens a dedicated TCP connection to the daemon, sends a Subscribe request
/// covering the full set of broadcast events, and forwards each Event /
/// LogMessage line as an SSE message. The browser receives a continuous
/// stream of typed JSON payloads as long as the connection is held open.
///
/// When the SSE client disconnects, the spawned task's send fails and the
/// task exits, dropping the daemon TCP connection.
async fn events_sse(
	State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
	let (tx, rx) = mpsc::channel::<String>(64);
	let socket_addr = state.socket_addr.clone();

	tokio::spawn(async move {
		if let Err(e) = bridge_daemon_events(socket_addr, tx).await {
			tracing::warn!("event bridge ended: {}", e);
		}
	});

	let stream = ReceiverStream::new(rx)
		.map(|line| Ok::<SseEvent, Infallible>(SseEvent::default().data(line)));

	Sse::new(stream).keep_alive(
		KeepAlive::new()
			.interval(Duration::from_secs(15))
			.text("keep-alive"),
	)
}

/// Connect to the daemon socket, subscribe to its event stream, and forward
/// each Event/LogMessage line into the channel. Returns Err on transport
/// failure or when the receiver is dropped.
async fn bridge_daemon_events(
	socket_addr: String,
	tx: mpsc::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let stream = TcpStream::connect(&socket_addr).await?;
	let (reader, mut writer) = stream.into_split();

	// Subscribe with empty event_types meaning "all", and no filter.
	let subscribe = serde_json::json!({
		"Subscribe": {
			"event_types": [],
			"filter": null,
		}
	});
	let line = serde_json::to_string(&subscribe)?;
	writer.write_all(line.as_bytes()).await?;
	writer.write_all(b"\n").await?;

	let mut reader = BufReader::new(reader);
	let mut buf = String::new();

	loop {
		buf.clear();
		let n = reader.read_line(&mut buf).await?;
		if n == 0 {
			// Daemon closed the connection.
			return Ok(());
		}
		let trimmed = buf.trim();
		if trimmed.is_empty() {
			continue;
		}

		// Forward only Event/LogMessage lines; skip Subscribed/Unsubscribed
		// acks and anything else the daemon might emit.
		match serde_json::from_str::<serde_json::Value>(trimmed) {
			Ok(value) => {
				let is_payload = value.get("Event").is_some() || value.get("LogMessage").is_some();
				if !is_payload {
					continue;
				}
				if tx.send(trimmed.to_string()).await.is_err() {
					// Receiver dropped — client disconnected.
					return Ok(());
				}
			}
			Err(e) => {
				tracing::debug!("daemon emitted non-JSON line: {}", e);
				continue;
			}
		}
	}
}

/// Proxy RPC requests to the daemon via TCP
async fn daemon_rpc(
	State(state): State<AppState>,
	Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
	// Connect to daemon
	let mut stream = TcpStream::connect(&state.socket_addr).await.map_err(|e| {
		(
			StatusCode::SERVICE_UNAVAILABLE,
			format!("Daemon not available: {}", e),
		)
	})?;

	// Send request
	let request_line = serde_json::to_string(&payload)
		.map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;

	stream
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.map_err(|e| {
			(
				StatusCode::INTERNAL_SERVER_ERROR,
				format!("Write failed: {}", e),
			)
		})?;

	// Read response
	let mut reader = BufReader::new(stream);
	let mut response_line = String::new();

	reader.read_line(&mut response_line).await.map_err(|e| {
		(
			StatusCode::INTERNAL_SERVER_ERROR,
			format!("Read failed: {}", e),
		)
	})?;

	// Parse and return
	let response: serde_json::Value = serde_json::from_str(&response_line).map_err(|e| {
		(
			StatusCode::INTERNAL_SERVER_ERROR,
			format!("Invalid response: {}", e),
		)
	})?;

	Ok(Json(response))
}

#[derive(Parser, Debug)]
#[command(name = "spacedrive-server", about = "Spacedrive HTTP server")]
struct Args {
	/// Path to spacedrive data directory
	#[arg(long, env = "DATA_DIR")]
	data_dir: Option<PathBuf>,

	/// Address to bind HTTP server. The wildcard default suits containers;
	/// bind a specific address (e.g. 127.0.0.1) when a reverse proxy such as
	/// `tailscale serve` owns the same port on other interfaces.
	#[arg(long, env = "SD_HOST", default_value = "::")]
	host: std::net::IpAddr,

	/// Port to bind HTTP server (default: 8080)
	#[arg(long, env = "PORT", default_value = "8080")]
	port: u16,

	/// Authentication credentials (format: "username:password,username2:password2")
	/// Set to "disabled" to disable auth (not recommended in production)
	#[arg(long, env = "SD_AUTH")]
	auth: Option<String>,

	/// Daemon instance name (for running multiple instances)
	#[arg(long)]
	instance: Option<String>,

	/// Enable P2P networking
	#[arg(long, env = "SD_P2P", default_value = "true")]
	p2p: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "info,sd_core=debug,sd_server=debug".into()),
		)
		.init();

	let args = Args::parse();

	// Resolve data directory
	let base_data_dir = args.data_dir.unwrap_or_else(|| {
		#[cfg(not(debug_assertions))]
		{
			std::env::var("DATA_DIR")
				.expect("DATA_DIR must be set in production")
				.into()
		}
		#[cfg(debug_assertions)]
		{
			// Default to `~/.spacedrive` in dev — matches the Tauri desktop app,
			// so `just dev-server` and `just dev-desktop` share libraries and
			// data persists between runs. Falls back to a tempdir only if the
			// home directory can't be resolved.
			std::env::var("DATA_DIR")
				.map(PathBuf::from)
				.or_else(|_| {
					dirs::home_dir()
						.map(|h| h.join(".spacedrive"))
						.ok_or(())
				})
				.unwrap_or_else(|_| {
					warn!("Could not resolve home directory; falling back to tempdir");
					let temp = tempfile::tempdir().expect("Failed to create temp dir");
					temp.path().to_path_buf()
				})
		}
	});

	// Calculate instance-specific paths
	let (data_dir, socket_addr) = if let Some(instance) = &args.instance {
		let instance_data_dir = sd_core::infra::daemon::addr::instance_data_dir(
			base_data_dir.clone(),
			Some(instance),
		);
		let socket_addr =
			sd_core::infra::daemon::addr::daemon_socket_addr(Some(instance)).to_string();
		(instance_data_dir, socket_addr)
	} else {
		let socket_addr = "127.0.0.1:6969".to_string();
		(base_data_dir.clone(), socket_addr)
	};

	info!("Data directory: {:?}", data_dir);
	info!("Socket address: {}", socket_addr);

	// Parse authentication
	let (auth, _disabled) = parse_auth(args.auth.as_deref());

	// Require credentials in production builds (unless explicitly disabled)
	#[cfg(not(debug_assertions))]
	if auth.is_empty() && !_disabled {
		warn!("The 'SD_AUTH' environment variable is not set!");
		warn!("If you want to disable auth set 'SD_AUTH=disabled', or");
		warn!("Provide your credentials in the following format 'SD_AUTH=username:password,username2:password2'");
		std::process::exit(1);
	}

	// Start the daemon if not already running
	let daemon_handle =
		start_daemon_if_needed(socket_addr.clone(), data_dir.clone(), args.p2p).await?;

	// Build HTTP router
	let state = AppState {
		auth,
		socket_addr: socket_addr.clone(),
		data_dir: data_dir.clone(),
	};

	let app = Router::new()
		.route("/health", get(health))
		.route("/rpc", post(daemon_rpc))
		.route("/events", get(events_sse))
		.route(
			"/sidecar/:library_id/:content_uuid/:kind/*variant",
			get(serve_sidecar),
		)
		.fallback(serve_web)
		.layer(middleware::from_fn_with_state(state.clone(), basic_auth))
		.with_state(state);

	// Bind server
	let addr = SocketAddr::new(args.host, args.port);

	// Setup graceful shutdown
	let shutdown_signal = shutdown_signal(daemon_handle);

	// Start server
	let listener = tokio::net::TcpListener::bind(addr).await?;
	info!("Spacedrive Server listening on http://{}", addr);
	info!("Web UI available at /");
	info!("RPC endpoint available at /rpc");
	axum::serve(listener, app)
		.with_graceful_shutdown(shutdown_signal)
		.await?;

	Ok(())
}

/// Parse authentication credentials from env var
fn parse_auth(auth_str: Option<&str>) -> (HashMap<String, SecStr>, bool) {
	let Some(input) = auth_str else {
		return (HashMap::new(), false);
	};

	if input == "disabled" {
		return (HashMap::new(), true);
	}

	let credentials = input
		.split(',')
		.enumerate()
		.filter_map(|(i, s)| {
			if s.is_empty() {
				return None;
			}

			let mut parts = s.split(':');
			let result = parts.next().and_then(|user| {
				parts
					.next()
					.map(|pass| (user.to_string(), SecStr::from(pass)))
			});

			if result.is_none() {
				warn!("Found invalid credential {i}. Skipping...");
			}
			result
		})
		.collect();

	(credentials, false)
}

/// Start the daemon if it's not already running
async fn start_daemon_if_needed(
	socket_addr: String,
	data_dir: PathBuf,
	enable_p2p: bool,
) -> Result<Option<Arc<RwLock<tokio::task::JoinHandle<()>>>>, Box<dyn std::error::Error>> {
	// Check if daemon is already running by sending a ping
	if is_daemon_running(&socket_addr).await {
		info!("✓ Daemon already running");
		return Ok(None);
	}

	info!("Starting embedded daemon...");

	// Start daemon in background task
	let socket_addr_clone = socket_addr.clone();
	let data_dir_clone = data_dir.clone();

	let handle = tokio::spawn(async move {
		if let Err(e) = sd_core::infra::daemon::bootstrap::start_default_server(
			socket_addr_clone,
			data_dir_clone,
			enable_p2p,
		)
		.await
		{
			tracing::error!("Daemon failed: {}", e);
		}
	});

	// Wait for daemon to be ready. Networking init (Iroh + relays) can take a
	// while when relays are unreachable, so we give it a generous window before
	// failing — better to wait than to spuriously crash on a flaky relay.
	for i in 0..300 {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		if TcpStream::connect(&socket_addr).await.is_ok() {
			info!("✓ Daemon started successfully");
			return Ok(Some(Arc::new(RwLock::new(handle))));
		}
		if i == 30 {
			warn!("Daemon taking longer than expected to start...");
		}
	}

	Err("Daemon failed to start (connection not available after 30 seconds)".into())
}

/// Check if daemon is running by sending a ping
async fn is_daemon_running(socket_addr: &str) -> bool {
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

	let mut stream = match TcpStream::connect(socket_addr).await {
		Ok(s) => s,
		Err(_) => return false,
	};

	let ping_request = serde_json::json!({"Ping": null});
	let request_line = match serde_json::to_string(&ping_request) {
		Ok(s) => s,
		Err(_) => return false,
	};

	if stream
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.is_err()
	{
		return false;
	}

	let (reader, _writer) = stream.into_split();
	let mut buf_reader = BufReader::new(reader);
	let mut response_line = String::new();

	matches!(
		tokio::time::timeout(
			tokio::time::Duration::from_millis(500),
			buf_reader.read_line(&mut response_line),
		)
		.await,
		Ok(Ok(_)) if !response_line.is_empty()
	)
}

/// Graceful shutdown handler.
///
/// On first signal: logs, aborts the daemon task (if we started it), and arms
/// a background force-exit that fires on either a second Ctrl+C or a 5-second
/// timeout. This returns immediately so axum can begin its graceful shutdown,
/// but long-held connections (e.g. the `/events` SSE stream held open by a
/// browser) can't block the process indefinitely.
async fn shutdown_signal(daemon_handle: Option<Arc<RwLock<tokio::task::JoinHandle<()>>>>) {
	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install signal handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	tokio::select! {
		() = ctrl_c => {
			info!("Received Ctrl+C, shutting down gracefully (5s timeout)...");
		}
		() = terminate => {
			info!("Received SIGTERM, shutting down gracefully (5s timeout)...");
		}
	}

	// Abort daemon task if we started it
	if let Some(handle) = daemon_handle {
		handle.write().await.abort();
	}

	// Force-exit fallback: if graceful shutdown doesn't finish (e.g. a browser
	// is holding an SSE connection open), exit on the next Ctrl+C or after 5s.
	tokio::spawn(async {
		tokio::select! {
			_ = signal::ctrl_c() => {
				warn!("Second signal received, forcing exit");
			}
			_ = tokio::time::sleep(Duration::from_secs(5)) => {
				warn!("Graceful shutdown timed out after 5s, forcing exit");
			}
		}
		std::process::exit(0);
	});
}
