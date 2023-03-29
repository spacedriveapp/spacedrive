use std::net::{SocketAddr, TcpListener};

use axum::{
	extract::{Query, State, TypedHeader},
	headers::authorization::{Authorization, Bearer},
	http::{Request, StatusCode},
	middleware::{self, Next},
	response::Response,
	routing::get,
	RequestPartsExt,
};
use httpz::{Endpoint, HttpEndpoint};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use tauri::{async_runtime::Receiver, plugin::TauriPlugin, Builder, Runtime};
use tracing::debug;

pub(super) async fn setup<R: Runtime>(
	app: Builder<R>,
	mut rx: Receiver<()>,
	endpoint: Endpoint<impl HttpEndpoint>,
) -> Builder<R> {
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
			.with_graceful_shutdown(async {
				rx.recv().await;
			})
			.await
			.expect("Error with HTTP server!");
	});

	app.plugin(tauri_plugin(&auth_token, listen_addr))
}

#[derive(Deserialize)]
struct QueryToken {
	token: String,
}

async fn auth_middleware<B>(
	Query(query): Query<QueryToken>,
	State(auth_token): State<String>,
	request: Request<B>,
	next: Next<B>,
) -> Result<Response, StatusCode>
where
	B: Send,
{
	let req = if query.token != auth_token {
		let (mut parts, body) = request.into_parts();

		let auth: TypedHeader<Authorization<Bearer>> = parts
			.extract()
			.await
			.map_err(|_| StatusCode::UNAUTHORIZED)?;

		if auth.token() != auth_token {
			return Err(StatusCode::UNAUTHORIZED);
		}

		Request::from_parts(parts, body)
	} else {
		request
	};

	Ok(next.run(req).await)
}

fn tauri_plugin<R: Runtime>(auth_token: &str, listen_addr: SocketAddr) -> TauriPlugin<R> {
	tauri::plugin::Builder::new("spacedrive-linux")
		.js_init_script(format!(
                r#"window.__SD_CUSTOM_SERVER_AUTH_TOKEN__ = "{auth_token}"; window.__SD_CUSTOM_URI_SERVER__ = "http://{listen_addr}";"#
		))
		.build()
}
