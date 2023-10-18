use std::sync::{atomic::AtomicBool, Arc};

use sd_p2p::{
	spaceblock::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer},
	spacetime::UnicastStream,
};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;
use uuid::Uuid;

use crate::{library::Library, p2p::Header};

/// Request a file from the remote machine over P2P. This is used for preview media and quick preview.
///
/// DO NOT USE THIS WITHOUT `node.files_over_p2p_flag == true`
pub async fn request_file(
	mut stream: UnicastStream,
	library: &Library,
	file_path_id: Uuid,
	range: Range,
	output: impl AsyncWrite + Unpin,
) {
	let id = Uuid::new_v4();
	// TODO: Tunnel for encryption + authentication

	stream
		.write_all(
			&Header::File {
				id,
				library_id: library.id,
				file_path_id,
				range: range.clone(),
			}
			.to_bytes(),
		)
		.await
		.unwrap();

	let block_size = BlockSize::from_stream(&mut stream).await.unwrap();
	let size = stream.read_u64_le().await.unwrap();

	Transfer::new(
		&SpaceblockRequests {
			id,
			block_size,
			requests: vec![SpaceblockRequest {
				// TODO: Removing need for this field in this case
				name: "todo".to_string(),
				// TODO: Maybe removing need for `size` from this side
				size,
				range,
			}],
		},
		|percent| {
			debug!(
				"P2P receiving file path '{}' - progress {}%",
				file_path_id, percent
			);
		},
		&Arc::new(AtomicBool::new(false)),
	)
	.receive(&mut stream, output)
	.await;
}

// TODO: Unit tests
