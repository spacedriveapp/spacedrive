use std::{
	net::{SocketAddr, TcpListener},
	sync::Arc,
};

use sd_core::Node;

use axum::{
	extract::State,
	http::{Request, StatusCode},
	middleware::{self, Next},
	response::{IntoResponse, Response},
	routing::get,
};
use httpz::{Endpoint, HttpEndpoint};
use rand::{distributions::Alphanumeric, Rng};
use tauri::{plugin::TauriPlugin, Builder, Runtime};
use tracing::debug;
use url::Url;

pub(super) async fn setup<R: Runtime>(
	app: Builder<R>,
	node: Arc<Node>,
	endpoint: Endpoint<impl HttpEndpoint>,
) -> Builder<R> {
	let signal = server::utils::axum_shutdown_signal(node);

	let auth_token: String = rand::thread_rng()
		.sample_iter(&Alphanumeric)
		.take(10)
		.map(char::from)
		.collect();

	let axum_app = axum::Router::new()
		.route("/", get(|| async { "Spacedrive Server!" }))
		.nest("/spacedrive", endpoint.axum())
		.route_layer(middleware::from_fn_with_state(
			auth_token.clone(),
			auth_middleware,
		))
		.fallback(|| async { "404 Not Found: We're past the event horizon..." });

	// Only allow current device to access it and randomise port
	let listener = TcpListener::bind("127.0.0.1:0").expect("Error creating localhost server!");
	let listen_addr = listener
		.local_addr()
		.expect("Error getting localhost server listen addr!");

	debug!("Localhost server listening on: http://{:?}", listen_addr);

	tokio::spawn(async move {
		axum::Server::from_tcp(listener)
			.expect("error creating HTTP server!")
			.serve(axum_app.into_make_service())
			.with_graceful_shutdown(signal)
			.await
			.expect("Error with HTTP server!");
	});

	app.plugin(spacedrive_plugin_init(&auth_token, listen_addr))
}

async fn auth_middleware<B>(
	State(auth_token): State<String>,
	request: Request<B>,
	next: Next<B>,
) -> Response {
	let url = Url::parse(&request.uri().to_string()).unwrap();
	if let Some((_, v)) = url.query_pairs().find(|(k, _)| k == "token") {
		if v == auth_token {
			return next.run(request).await;
		}
	} else if let Some(v) = request
		.headers()
		.get("Authorization")
		.and_then(|v| v.to_str().ok())
	{
		if v == auth_token {
			return next.run(request).await;
		}
	}

	(StatusCode::UNAUTHORIZED, "Unauthorized!").into_response()
}

pub fn spacedrive_plugin_init<R: Runtime>(
	auth_token: &str,
	listen_addr: SocketAddr,
) -> TauriPlugin<R> {
	tauri::plugin::Builder::new("spacedrive")
		.js_init_script(format!(
                r#"window.__SD_CUSTOM_SERVER_AUTH_TOKEN__ = "{auth_token}"; window.__SD_CUSTOM_URI_SERVER__ = "http://{listen_addr}";"#
		))
		.build()
}
