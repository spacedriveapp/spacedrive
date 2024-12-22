//! A HTTP response builder similar to the [http] crate but designed to be infallible.

use axum::http::{
	self, header::IntoHeaderName, response::Parts, HeaderValue, Response, StatusCode,
};

#[derive(Debug)]
pub struct InfallibleResponse(Parts);

impl InfallibleResponse {
	pub fn builder() -> Self {
		Self(Response::new(()).into_parts().0)
	}

	pub fn status(mut self, status: StatusCode) -> Self {
		self.0.status = status;
		self
	}

	pub fn header<K: IntoHeaderName>(mut self, key: K, val: HeaderValue) -> Self {
		self.0.headers.insert(key, val);
		self
	}

	pub fn body<B>(self, body: B) -> http::Response<B> {
		Response::from_parts(self.0, body)
	}
}
