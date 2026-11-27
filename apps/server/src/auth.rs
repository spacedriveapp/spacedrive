//! # HTTP Basic authentication
//!
//! Every route sits behind this middleware. Credentials come from `SD_AUTH`
//! and are compared in constant time against every configured user, so
//! response timing reveals neither a valid username nor a password prefix.
//! Failed attempts feed the [`LoginLimiter`], which answers 429 to a locked
//! out address before its credentials are looked at.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
	extract::{ConnectInfo, FromRequestParts, Request, State},
	http::{header, StatusCode},
	middleware::Next,
	response::{IntoResponse, Response},
};
use axum_extra::{headers::authorization::Basic, headers::Authorization, TypedHeader};
use secstr::SecStr;
use subtle::{Choice, ConstantTimeEq};

use crate::login_limit::LoginLimiter;

#[derive(Clone)]
pub struct AuthState {
	pub credentials: Arc<HashMap<String, SecStr>>,
	pub limiter: Arc<LoginLimiter>,
}

impl AuthState {
	pub fn new(credentials: HashMap<String, SecStr>) -> Self {
		Self {
			credentials: Arc::new(credentials),
			limiter: Arc::new(LoginLimiter::default()),
		}
	}

	fn verify(&self, user: &str, password: &str) -> bool {
		self.credentials
			.iter()
			.fold(Choice::from(0), |matched, (u, p)| {
				matched
					| (u.as_bytes().ct_eq(user.as_bytes())
						& p.unsecure().ct_eq(password.as_bytes()))
			})
			.into()
	}
}

fn unauthorized() -> Response {
	(
		StatusCode::UNAUTHORIZED,
		[(header::WWW_AUTHENTICATE, "Basic realm=\"Spacedrive\"")],
		"Unauthorized",
	)
		.into_response()
}

fn too_many_attempts(retry_after: std::time::Duration) -> Response {
	let secs = retry_after.as_secs() + u64::from(retry_after.subsec_nanos() > 0);
	(
		StatusCode::TOO_MANY_REQUESTS,
		[(header::RETRY_AFTER, secs.to_string())],
		"Too many failed logins",
	)
		.into_response()
}

/// Basic auth middleware, rate limited per TCP peer address.
pub async fn basic_auth(
	State(state): State<AuthState>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	request: Request,
	next: Next,
) -> Response {
	if state.credentials.is_empty() {
		return next.run(request).await;
	}
	if let Err(retry_after) = state.limiter.check(peer.ip()) {
		return too_many_attempts(retry_after);
	}

	let (mut parts, body) = request.into_parts();
	// A request without credentials is a browser asking for the prompt, not a guess.
	let Ok(TypedHeader(Authorization(hdr))) =
		TypedHeader::<Authorization<Basic>>::from_request_parts(&mut parts, &()).await
	else {
		return unauthorized();
	};
	if !state.verify(hdr.username(), hdr.password()) {
		state.limiter.record_failure(peer.ip());
		return unauthorized();
	}

	next.run(Request::from_parts(parts, body)).await
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use axum::{body::Body, middleware, routing::get, Router};
	use base64::prelude::*;
	use tower::ServiceExt;

	use super::*;
	use crate::login_limit::{LOCKOUT, MAX_FAILURES, WINDOW};

	fn app(limiter: LoginLimiter) -> Router {
		let state = AuthState {
			credentials: Arc::new(HashMap::from([(
				"admin".to_string(),
				SecStr::from("secret"),
			)])),
			limiter: Arc::new(limiter),
		};
		Router::new()
			.route("/", get(|| async { "ok" }))
			.layer(middleware::from_fn_with_state(state, basic_auth))
	}

	async fn call(app: &Router, peer: &str, credentials: Option<&str>) -> Response {
		let mut request = Request::builder().uri("/");
		if let Some(credentials) = credentials {
			let encoded = BASE64_STANDARD.encode(credentials);
			request = request.header(header::AUTHORIZATION, format!("Basic {encoded}"));
		}
		let mut request = request.body(Body::empty()).unwrap();
		request
			.extensions_mut()
			.insert(ConnectInfo::<SocketAddr>(peer.parse().unwrap()));
		app.clone().oneshot(request).await.unwrap()
	}

	fn retry_after(response: &Response) -> u64 {
		response.headers()[header::RETRY_AFTER]
			.to_str()
			.unwrap()
			.parse()
			.unwrap()
	}

	#[tokio::test]
	async fn correct_and_missing_credentials() {
		let app = app(LoginLimiter::default());
		let peer = "198.51.100.1:5000";
		assert_eq!(
			call(&app, peer, Some("admin:secret")).await.status(),
			StatusCode::OK
		);
		let missing = call(&app, peer, None).await;
		assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
		assert!(missing.headers().contains_key(header::WWW_AUTHENTICATE));
		assert_eq!(
			call(&app, peer, Some("admin:secreT")).await.status(),
			StatusCode::UNAUTHORIZED
		);
		assert_eq!(
			call(&app, peer, Some("other:secret")).await.status(),
			StatusCode::UNAUTHORIZED
		);
	}

	#[tokio::test]
	async fn twenty_failures_then_lockout() {
		let app = app(LoginLimiter::default());
		let attacker = "203.0.113.7:40000";
		for _ in 0..MAX_FAILURES {
			assert_eq!(
				call(&app, attacker, Some("admin:wrong")).await.status(),
				StatusCode::UNAUTHORIZED
			);
		}
		let last = call(&app, attacker, Some("admin:wrong")).await;
		// The 21st attempt is refused before its credentials are read.
		assert_eq!(last.status(), StatusCode::TOO_MANY_REQUESTS);
		assert_eq!(retry_after(&last), LOCKOUT.as_secs());

		let next = call(&app, attacker, Some("admin:wrong")).await;
		assert_eq!(next.status(), StatusCode::TOO_MANY_REQUESTS);
		assert!(retry_after(&next) > 0 && retry_after(&next) <= LOCKOUT.as_secs());

		let correct = call(&app, "203.0.113.7:40001", Some("admin:secret")).await;
		assert_eq!(correct.status(), StatusCode::TOO_MANY_REQUESTS);
		assert!(correct.headers().contains_key(header::RETRY_AFTER));

		assert_eq!(
			call(&app, "203.0.113.8:40000", Some("admin:secret"))
				.await
				.status(),
			StatusCode::OK
		);
		assert_eq!(
			call(&app, "203.0.113.8:40000", Some("admin:wrong"))
				.await
				.status(),
			StatusCode::UNAUTHORIZED
		);
	}

	#[tokio::test]
	async fn lockout_ends() {
		let lockout = Duration::from_millis(300);
		let app = app(LoginLimiter::new(MAX_FAILURES, WINDOW, lockout, 16));
		let peer = "203.0.113.7:40000";
		for _ in 0..MAX_FAILURES {
			call(&app, peer, Some("admin:wrong")).await;
		}
		assert_eq!(
			call(&app, peer, Some("admin:secret")).await.status(),
			StatusCode::TOO_MANY_REQUESTS
		);
		tokio::time::sleep(lockout + Duration::from_millis(50)).await;
		assert_eq!(
			call(&app, peer, Some("admin:secret")).await.status(),
			StatusCode::OK
		);
	}

	#[tokio::test]
	async fn disabled_auth_skips_the_limiter() {
		let state = AuthState::new(HashMap::new());
		let app = Router::new()
			.route("/", get(|| async { "ok" }))
			.layer(middleware::from_fn_with_state(state, basic_auth));
		assert_eq!(
			call(&app, "203.0.113.7:1", None).await.status(),
			StatusCode::OK
		);
	}
}
