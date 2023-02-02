use std::{env, net::SocketAddr, path::Path};

use axum::{
	body::{Body, Full},
	http::Request,
	response::Response,
	routing::get,
};
use hyper::body::to_bytes;
use sd_core::{custom_uri::handle_custom_uri, Node};
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
		.route("/spacedrive/*id", {
			let node = node.clone();
			get(|req: Request<Body>| async move {
				let (parts, body) = req.into_parts();
				let mut r =
					Request::builder().method(parts.method).uri(
						parts.uri.path().strip_prefix("/spacedrive").expect(
							"Error decoding Spacedrive URL prefix. This should be impossible!",
						),
					);
				for (key, value) in parts.headers {
					if let Some(key) = key {
						r = r.header(key, value);
					}
				}
				let r = r.body(to_bytes(body).await.unwrap().to_vec()).unwrap();

				let resp = handle_custom_uri(&node, r)
					.await
					.unwrap_or_else(|err| err.into_response().unwrap());

				let mut r = Response::builder()
					.version(resp.version())
					.status(resp.status());

				for (key, value) in resp.headers() {
					r = r.header(key, value);
				}

				r.body(Full::from(resp.into_body())).unwrap()
			})
		})
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
