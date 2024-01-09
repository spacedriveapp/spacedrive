use std::{env, net::SocketAddr, path::Path};

use axum::routing::get;
use sd_core::{custom_uri, Node};
use tracing::info;

mod utils;

#[cfg(feature = "assets")]
static ASSETS_DIR: include_dir::Dir<'static> =
	include_dir::include_dir!("$CARGO_MANIFEST_DIR/../web/dist");

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
				Path::new(env!("CARGO_MANIFEST_DIR")).join("sdserver_data")
			}
		}
	};

	let port = env::var("PORT")
		.map(|port| port.parse::<u16>().unwrap_or(8080))
		.unwrap_or(8080);

	let _guard = match Node::init_logger(&data_dir) {
		Ok(guard) => guard,
		Err(e) => {
			panic!("{}", e.to_string())
		}
	};

	let (node, router) = match Node::new(
		data_dir,
		sd_core::Env {
			api_url: tokio::sync::Mutex::new(
				std::env::var("SD_API_URL")
					.unwrap_or_else(|_| "https://app.spacedrive.com".to_string()),
			),
			client_id: std::env::var("SD_CLIENT_ID")
				.unwrap_or_else(|_| "04701823-a498-406e-aef9-22081c1dae34".to_string()),
		},
	)
	.await
	{
		Ok(d) => d,
		Err(e) => {
			panic!("{}", e.to_string())
		}
	};
	let signal = utils::axum_shutdown_signal(node.clone());

	let app = axum::Router::new()
		.route("/health", get(|| async { "OK" }))
		.nest("/spacedrive", custom_uri::router(node.clone()))
		.nest("/rspc", router.endpoint(move || node.clone()).axum());

	#[cfg(feature = "assets")]
	let app = app
		.route(
			"/",
			get(|| async move {
				use axum::{
					body::{self, Full},
					response::Response,
				};
				use http::{header, HeaderValue, StatusCode};

				match ASSETS_DIR.get_file("index.html") {
					Some(file) => Response::builder()
						.status(StatusCode::OK)
						.header(
							header::CONTENT_TYPE,
							HeaderValue::from_str("text/html").unwrap(),
						)
						.body(body::boxed(Full::from(file.contents())))
						.unwrap(),
					None => Response::builder()
						.status(StatusCode::NOT_FOUND)
						.body(body::boxed(axum::body::Empty::new()))
						.unwrap(),
				}
			}),
		)
		.route(
			"/*id",
			get(
				|axum::extract::Path(path): axum::extract::Path<String>| async move {
					use axum::{
						body::{self, Empty, Full},
						response::Response,
					};
					use http::{header, HeaderValue, StatusCode};

					let path = path.trim_start_matches('/');
					match ASSETS_DIR.get_file(path) {
						Some(file) => Response::builder()
							.status(StatusCode::OK)
							.header(
								header::CONTENT_TYPE,
								HeaderValue::from_str(
									mime_guess::from_path(path).first_or_text_plain().as_ref(),
								)
								.unwrap(),
							)
							.body(body::boxed(Full::from(file.contents())))
							.unwrap(),
						None => match ASSETS_DIR.get_file("index.html") {
							Some(file) => Response::builder()
								.status(StatusCode::OK)
								.header(
									header::CONTENT_TYPE,
									HeaderValue::from_str("text/html").unwrap(),
								)
								.body(body::boxed(Full::from(file.contents())))
								.unwrap(),
							None => Response::builder()
								.status(StatusCode::NOT_FOUND)
								.body(body::boxed(Empty::new()))
								.unwrap(),
						},
					}
				},
			),
		);

	#[cfg(not(feature = "assets"))]
	let app = app
		.route("/", get(|| async { "Spacedrive Server!" }))
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
