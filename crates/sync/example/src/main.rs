use axum::{
	http::{HeaderValue, Method},
	routing::get,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

mod api;
// mod prisma;
mod utils;

fn router() -> axum::Router {
	let router = api::new().build().arced();

	let ctx = Arc::new(Mutex::new(Default::default()));

	axum::Router::new()
		.route("/", get(|| async { "Hello 'rspc'!" }))
		.route("/rspc/:id", router.endpoint(move || ctx.clone()).axum())
		.layer(
			CorsLayer::new()
				.allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
				.allow_headers(vec![http::header::CONTENT_TYPE])
				.allow_methods([Method::GET, Method::POST]),
		)
}

#[tokio::main]
async fn main() {
	dotenv::dotenv().ok();

	let addr = "[::]:9000".parse::<SocketAddr>().unwrap(); // This listens on IPv6 and IPv4
	println!("{} listening on http://{}", env!("CARGO_CRATE_NAME"), addr);
	axum::Server::bind(&addr)
		.serve(router().into_make_service())
		.with_graceful_shutdown(utils::axum_shutdown_signal())
		.await
		.expect("Error with HTTP server!");
}
