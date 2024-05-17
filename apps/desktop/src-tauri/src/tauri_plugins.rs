use std::{
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
use http::Method;
use hyper::server::{accept::Accept, conn::AddrIncoming};
use rand::{distributions::Alphanumeric, Rng};
use sd_core::{custom_uri, Node, NodeError};
use serde::Deserialize;
use tauri::{async_runtime::block_on, plugin::TauriPlugin, RunEvent, Runtime};
use thiserror::Error;
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

#[derive(Error, Debug)]
pub enum SdServerPluginError {
	#[error("hyper error")]
	HyperError(#[from] hyper::Error),
	#[error("io error")]
	IoError(#[from] std::io::Error),
}

/// Right now Tauri doesn't support async custom URI protocols so we ship an Axum server.
/// I began the upstream work on this: https://github.com/tauri-apps/wry/pull/872
/// Related to https://github.com/tauri-apps/tauri/issues/3725 & https://bugs.webkit.org/show_bug.cgi?id=146351#c5
///
/// The server is on a random port w/ a localhost bind address and requires a random on startup auth token which is injected into the webview so this *should* be secure enough.
///
/// We also spin up multiple servers so we can load balance image requests between them to avoid any issue with browser connection limits.
pub async fn sd_server_plugin<R: Runtime>(
	node: Arc<Node>,
) -> Result<TauriPlugin<R>, SdServerPluginError> {
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

	// Only allow current device to access it
	let listenera = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
	let listen_addra = listenera.local_addr()?;
	let listenerb = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
	let listen_addrb = listenerb.local_addr()?;
	let listenerc = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
	let listen_addrc = listenerc.local_addr()?;
	let listenerd = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
	let listen_addrd = listenerd.local_addr()?;

	// let listen_addr = listener.local_addr()?; // We get it from a listener so `0` is turned into a random port
	let (tx, mut rx) = tokio::sync::mpsc::channel(1);

	info!("Internal server listening on: http://{listen_addra:?} http://{listen_addrb:?} http://{listen_addrc:?} http://{listen_addrd:?}");
	let server = axum::Server::builder(CombinedIncoming {
		a: AddrIncoming::from_listener(listenera)?,
		b: AddrIncoming::from_listener(listenerb)?,
		c: AddrIncoming::from_listener(listenerc)?,
		d: AddrIncoming::from_listener(listenerd)?,
	});
	tokio::spawn(async move {
		server
			.serve(app.into_make_service())
			.with_graceful_shutdown(async {
				rx.recv().await;
			})
			.await
			.expect("Error with HTTP server!"); // TODO: Panic handling
	});

	let script = format!(
		r#"window.__SD_CUSTOM_SERVER_AUTH_TOKEN__ = "{auth_token}"; window.__SD_CUSTOM_URI_SERVER__ = [{}];"#,
		[listen_addra, listen_addrb, listen_addrc, listen_addrd]
			.iter()
			.map(|addr| format!("'http://{addr}'"))
			.collect::<Vec<_>>()
			.join(","),
	);

	Ok(tauri::plugin::Builder::new("sd-server")
		.js_init_script(script.to_owned())
		.on_page_load(move |webview, _payload| {
			webview
				.eval(&script)
				.expect("Spacedrive server URL must be injected")
		})
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
	token: Option<String>,
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
	let req = if query.token.as_ref() != Some(&auth_token) {
		let (mut parts, body) = request.into_parts();

		// We don't check auth for OPTIONS requests cause the CORS middleware will handle it
		if parts.method != Method::OPTIONS {
			let auth: TypedHeader<Authorization<Bearer>> = parts
				.extract()
				.await
				.map_err(|_| StatusCode::UNAUTHORIZED)?;

			if auth.token() != auth_token {
				return Err(StatusCode::UNAUTHORIZED);
			}
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
