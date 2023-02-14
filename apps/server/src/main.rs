use std::{env, net::SocketAddr, path::Path};

use axum::routing::get;
use sd_core::{custom_uri::create_custom_uri_endpoint, Node};
use tracing::info;

mod utils;

#[tokio::main]
async fn main() {
	let data_dir = match env::var("DATA_DIR") {
		Ok(path) => Path::new(&path).to_path_buf(),
		Err(_e) => {
			#[cfg(not(debug_assertions))]
			{
				panic!("'$DATA_DIR' is not set ({})", _e)
			}
			#[cfg(debug_assertions)]
			{
				std::env::current_dir()
					.expect("Unable to get your current directory. Maybe try setting $DATA_DIR?")
					.join("sdserver_data")
			}
		}
	};

	let port = env::var("PORT")
		.map(|port| port.parse::<u16>().unwrap_or(8080))
		.unwrap_or(8080);

	let (node, router) = Node::new(data_dir).await.expect("Unable to create node");
	let signal = utils::axum_shutdown_signal(node.clone());

	let app = axum::Router::new()
		.route("/", get(|| async { "Spacedrive Server!" }))
		.route("/health", get(|| async { "OK" }))
		.nest(
			"/spacedrive",
			create_custom_uri_endpoint(node.clone()).axum(),
		)
		.nest(
			"/rspc",
			router.endpoint(move || node.get_request_context()).axum(),
		)
		.fallback(|| async { "404 Not Found: We're past the event horizon..." });

	let mut addr = "[::]:8080".parse::<SocketAddr>().unwrap(); // This listens on IPv6 and IPv4
	addr.set_port(port);
	info!("Listening on http://localhost:{}", port);
	axum::Server::bind(&addr)
		.serve(app.into_make_service())
		.with_graceful_shutdown(signal)
		.await
		.expect("Error with HTTP server!");
}
