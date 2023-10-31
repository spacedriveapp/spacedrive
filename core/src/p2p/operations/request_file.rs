use std::{
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use sd_p2p::{
	spaceblock::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer},
	spacetime::UnicastStream,
	PeerMessageEvent,
};
use sd_prisma::prisma::file_path;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};
use tracing::debug;
use uuid::Uuid;

use crate::{
	library::Library,
	location::file_path_helper::{file_path_to_handle_p2p_serve_file, IsolatedFilePathData},
	p2p::{Header, HeaderFile},
	Node,
};

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
			&Header::File(HeaderFile {
				id,
				library_id: library.id,
				file_path_id,
				range: range.clone(),
			})
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

pub(crate) async fn receiver(
	node: &Arc<Node>,
	HeaderFile {
		id,
		library_id,
		file_path_id,
		range,
	}: HeaderFile,
	event: PeerMessageEvent,
) -> Result<(), ()> {
	let mut stream = event.stream;
	if !node.files_over_p2p_flag.load(Ordering::Relaxed) {
		panic!("Files over P2P is disabled!");
	}

	// TODO: Tunnel and authentication
	// TODO: Use BufReader

	let library = node.libraries.get_library(&library_id).await.unwrap();

	let file_path = library
		.db
		.file_path()
		.find_unique(file_path::pub_id::equals(file_path_id.as_bytes().to_vec()))
		.select(file_path_to_handle_p2p_serve_file::select())
		.exec()
		.await
		.unwrap()
		.unwrap();

	let location = file_path.location.as_ref().unwrap();
	let location_path = location.path.as_ref().unwrap();
	let path = Path::new(location_path)
		.join(IsolatedFilePathData::try_from((location.id, &file_path)).unwrap());

	debug!("Serving path '{:?}' over P2P", path);

	let file = File::open(&path).await.unwrap();

	let metadata = file.metadata().await.unwrap();
	let block_size = BlockSize::from_size(metadata.len());

	stream.write_all(&block_size.to_bytes()).await.unwrap();
	stream
		.write_all(&metadata.len().to_le_bytes())
		.await
		.unwrap();

	let file = BufReader::new(file);
	Transfer::new(
		&SpaceblockRequests {
			id,
			block_size,
			requests: vec![SpaceblockRequest {
				// TODO: Removing need for this field in this case
				name: "todo".to_string(),
				size: metadata.len(),
				range,
			}],
		},
		|percent| {
			debug!(
				"P2P loading file path '{}' - progress {}%",
				file_path_id, percent
			);
		},
		&Arc::new(AtomicBool::new(false)),
	)
	.send(&mut stream, file)
	.await;

	Ok(())
}

// TODO: Unit tests
