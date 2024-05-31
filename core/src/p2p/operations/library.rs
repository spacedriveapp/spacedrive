use std::{
	error::Error,
	path::Path,
	sync::{atomic::AtomicBool, Arc},
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_to_handle_p2p_serve_file;
use sd_p2p::{Identity, RemoteIdentity, UnicastStream, P2P};
use sd_p2p_block::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer};
use sd_prisma::prisma::file_path;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};
use tracing::debug;
use uuid::Uuid;

use crate::{p2p::Header, Node};

/// Request a file from a remote library
#[allow(unused)]
pub async fn request_file(
	p2p: Arc<P2P>,
	identity: RemoteIdentity,
	library_identity: &Identity,
	file_path_id: Uuid,
	range: Range,
	output: impl AsyncWrite + Unpin,
) -> Result<(), Box<dyn Error>> {
	let peer = p2p.peers().get(&identity).ok_or("Peer offline")?.clone();
	let mut stream = peer.new_stream().await?;

	stream
		.write_all(
			&Header::LibraryFile {
				file_path_id,
				range: range.clone(),
			}
			.to_bytes(),
		)
		.await?;

	let mut stream = sd_p2p_tunnel::Tunnel::initiator(stream, library_identity).await?;

	let block_size = BlockSize::from_stream(&mut stream).await?;
	let size = stream.read_u64_le().await?;

	Transfer::new(
		&SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "_".to_string(),
				size,
				range,
			}],
		},
		|percent| debug!("P2P receiving file path {file_path_id:?} - progress {percent}%"),
		&Arc::new(AtomicBool::new(false)),
	)
	.receive(&mut stream, output)
	.await;

	Ok(())
}

pub(crate) async fn receiver(
	stream: UnicastStream,
	file_path_id: Uuid,
	range: Range,
	node: &Arc<Node>,
) -> Result<(), Box<dyn Error>> {
	debug!(
		"Received library request from peer '{}'",
		stream.remote_identity()
	);

	// The tunnel takes care of authentication and encrypts all traffic to the library to be certain we are talking to a node with the library.
	let mut stream = sd_p2p_tunnel::Tunnel::responder(stream).await?;

	let library = node
		.libraries
		.get_library_for_instance(&stream.library_remote_identity())
		.await
		.ok_or_else(|| format!("Library not found: {:?}", stream.library_remote_identity()))?;

	let file_path = library
		.db
		.file_path()
		.find_unique(file_path::pub_id::equals(file_path_id.as_bytes().to_vec()))
		.select(file_path_to_handle_p2p_serve_file::select())
		.exec()
		.await?
		.ok_or_else(|| format!("File path {file_path_id:?} not found in {:?}", library.id))?;

	let location = file_path.location.as_ref().expect("included in query");
	let location_path = location.path.as_ref().expect("included in query");
	let path =
		Path::new(location_path).join(IsolatedFilePathData::try_from((location.id, &file_path))?);

	debug!(
		"Serving path {path:?} for library {:?} over P2P",
		library.id
	);

	let file = File::open(&path).await?;

	let metadata = file.metadata().await?;
	let block_size = BlockSize::from_file_size(metadata.len());

	stream.write_all(&block_size.to_bytes()).await?;
	stream.write_all(&metadata.len().to_le_bytes()).await?;

	let file = BufReader::new(file);
	Transfer::new(
		&SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "_".into(),
				size: metadata.len(),
				range,
			}],
		},
		|percent| debug!("P2P loading file path {file_path_id:?} - progress {percent}%"),
		// TODO: Properly handle cancellation with webview
		&Arc::new(AtomicBool::new(false)),
	)
	.send(&mut stream, file)
	.await?;

	Ok(())
}
