use axum::{
	extract::{FromRequestParts, Request, State},
	http::StatusCode,
	middleware::{self, Next},
	response::{IntoResponse, Response},
	routing::{get, post},
	Json, Router,
};
use axum_extra::{headers::authorization::Basic, headers::Authorization, TypedHeader};
use clap::Parser;
use secstr::SecStr;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
	io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
	net::TcpStream,
	signal,
	sync::RwLock,
};
use tracing::{info, warn};

#[derive(Clone)]
struct AppState {
	auth: HashMap<String, SecStr>,
	socket_addr: String,
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
			std::env::var("DATA_DIR")
				.map(PathBuf::from)
				.unwrap_or_else(|_| {
					let temp = tempfile::tempdir().expect("Failed to create temp dir");
					temp.path().to_path_buf()
				})
		}
	});

	// Calculate instance-specific paths
	let (data_dir, socket_addr) = if let Some(instance) = &args.instance {
		let instance_data_dir = base_data_dir.join("instances").join(instance);
		let port = 6970 + (instance.bytes().map(|b| b as u16).sum::<u16>() % 1000);
		let socket_addr = format!("127.0.0.1:{}", port);
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
	};

	let app = Router::new()
		.route("/health", get(health))
		.route("/rpc", post(daemon_rpc))
		.route(
			"/",
			get(|| async { "Spacedrive Server - RPC only (no web UI)" }),
		)
		.fallback(|| async {
			(
				StatusCode::NOT_FOUND,
				"404 Not Found: We're past the event horizon...",
			)
		})
		.layer(middleware::from_fn_with_state(state.clone(), basic_auth))
		.with_state(state);

	// Bind server
	let mut addr = "[::]:8080".parse::<SocketAddr>().unwrap();
	addr.set_port(args.port);

	info!(
		"Spacedrive Server listening on http://localhost:{}",
		args.port
	);
	info!("RPC endpoint available at /rpc");

	// Setup graceful shutdown
	let shutdown_signal = shutdown_signal(daemon_handle);

	// Start server
	let listener = tokio::net::TcpListener::bind(addr).await?;
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

	// Wait for daemon to be ready
	for i in 0..30 {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		if TcpStream::connect(&socket_addr).await.is_ok() {
			info!("✓ Daemon started successfully");
			return Ok(Some(Arc::new(RwLock::new(handle))));
		}
		if i == 10 {
			warn!("Daemon taking longer than expected to start...");
		}
	}

	Err("Daemon failed to start (connection not available after 3 seconds)".into())
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

/// Graceful shutdown handler
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
			info!("Received Ctrl+C, shutting down gracefully...");
		}
		() = terminate => {
			info!("Received SIGTERM, shutting down gracefully...");
		}
	}

	// Abort daemon task if we started it
	if let Some(handle) = daemon_handle {
		handle.write().await.abort();
	}
}
