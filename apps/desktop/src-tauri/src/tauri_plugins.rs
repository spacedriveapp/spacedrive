use std::{
	io,
	net::Ipv4Addr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use axum::{
	extract::{Query, State, TypedHeader},
	headers::authorization::{Authorization, Bearer},
	http::{Request, StatusCode},
	middleware::{self, Next},
	response::Response,
	RequestPartsExt,
};
use hyper::server::{accept::Accept, conn::AddrIncoming};
use rand::{distributions::Alphanumeric, Rng};
use sd_core::{custom_uri, Node, NodeError};
use serde::Deserialize;
use tauri::{async_runtime::block_on, plugin::TauriPlugin, RunEvent, Runtime};
use tokio::{net::TcpListener, task::block_in_place};
use tracing::info;

/// Inject `window.__SD_ERROR__` so the frontend can render core startup errors.
/// It's assumed the error happened prior or during settings up the core and rspc.
pub fn sd_error_plugin<R: Runtime>(err: NodeError) -> TauriPlugin<R> {
	tauri::plugin::Builder::new("sd-error")
		.js_init_script(format!(
			r#"window.__SD_ERROR__ = `{}`;"#,
			err.to_string().replace('`', "\"")
		))
		.build()
}

/// Right now Tauri doesn't support async custom URI protocols so we ship an Axum server.
/// I began the upstream work on this: https://github.com/tauri-apps/wry/pull/872
/// Related to https://github.com/tauri-apps/tauri/issues/3725 & https://bugs.webkit.org/show_bug.cgi?id=146351#c5
///
/// The server is on a random port w/ a localhost bind address and requires a random on startup auth token which is injected into the webview so this *should* be secure enough.
pub async fn sd_server_plugin<R: Runtime>(node: Arc<Node>) -> io::Result<TauriPlugin<R>> {
	let auth_token: String = rand::thread_rng()
		.sample_iter(&Alphanumeric)
		.take(15)
		.map(char::from)
		.collect();

	let app = custom_uri::router(node.clone())
		.route_layer(middleware::from_fn_with_state(
			auth_token.clone(),
			auth_middleware,
		))
		.fallback(|| async { "404 Not Found: We're past the event horizon..." });

	let port = std::env::var("SD_PORT")
		.ok()
		.and_then(|port| port.parse().ok())
		.unwrap_or(0); // randomise port

	// Only allow current device to access it
	let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
		.await
		.unwrap(); // TODO: Error handling
	let listen_addr = listener.local_addr()?; // We get it from a listener so `0` is turned into a random port
	let (tx, mut rx) = tokio::sync::mpsc::channel(1);

	info!("Internal server listening on: http://{:?}", listen_addr);
	tokio::spawn(async move {
		axum::Server::builder(CombinedIncoming {
			// TODO: Error handling
			a: AddrIncoming::from_listener(listener).unwrap(),
			// TODO: Ensure these ports aren't taken
			b: AddrIncoming::bind(&(Ipv4Addr::LOCALHOST, listen_addr.port() + 1).into()).unwrap(),
			c: AddrIncoming::bind(&(Ipv4Addr::LOCALHOST, listen_addr.port() + 2).into()).unwrap(),
			d: AddrIncoming::bind(&(Ipv4Addr::LOCALHOST, listen_addr.port() + 3).into()).unwrap(),
		})
		.serve(app.into_make_service())
		.with_graceful_shutdown(async {
			rx.recv().await;
		})
		.await
		.expect("Error with HTTP server!"); // TODO: Panic handling
	});

	Ok(tauri::plugin::Builder::new("sd-server")
		.js_init_script(format!(
		        r#"window.__SD_CUSTOM_SERVER_AUTH_TOKEN__ = "{auth_token}"; window.__SD_CUSTOM_URI_SERVER__ = "http://{listen_addr}"; window.__SD_START_PORT__ = {};"#,
				listen_addr.port(),
		))
		.on_event(move |_app, e| {
			if let RunEvent::Exit { .. } = e {
				block_in_place(|| {
					block_on(node.shutdown());
					block_on(tx.send(())).ok();
				});
			}
		})
		.build())
}

#[derive(Deserialize)]
struct QueryParams {
	token: String,
}

async fn auth_middleware<B>(
	Query(query): Query<QueryParams>,
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

struct CombinedIncoming {
	a: AddrIncoming,
	b: AddrIncoming,
	c: AddrIncoming,
	d: AddrIncoming,
}

impl Accept for CombinedIncoming {
	type Conn = <AddrIncoming as Accept>::Conn;
	type Error = <AddrIncoming as Accept>::Error;

	fn poll_accept(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
		if let Poll::Ready(Some(value)) = Pin::new(&mut self.a).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		if let Poll::Ready(Some(value)) = Pin::new(&mut self.b).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		if let Poll::Ready(Some(value)) = Pin::new(&mut self.c).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		if let Poll::Ready(Some(value)) = Pin::new(&mut self.d).poll_accept(cx) {
			return Poll::Ready(Some(value));
		}

		Poll::Pending
	}
}
