mod client;
mod transport;
mod types;

pub use client::SpacedriveClient;
pub use types::*;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_thumbnail_url_construction() {
		let client =
			SpacedriveClient::new("/tmp/test.sock".into(), "http://localhost:54321".into());

		let url = client.thumbnail_url("0cc0b48f-a475-53ec-a580-bc7d47b486a9", "grid@1x", "webp");

		assert_eq!(
            url,
            "http://localhost:54321/sidecar/None/0cc0b48f-a475-53ec-a580-bc7d47b486a9/thumb/grid@1x.webp"
        );
	}
}
