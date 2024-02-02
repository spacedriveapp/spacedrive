use crate::{
	library::Library,
	p2p::{Header, HeaderFile},
	Node,
};

use sd_file_path_helper::{file_path_to_handle_p2p_serve_file, IsolatedFilePathData};
use sd_p2p2::UnicastStream;
use sd_p2p_block::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer};
use sd_prisma::prisma::file_path;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};
use tracing::{debug, warn};
use uuid::Uuid;

use std::{
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
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
) -> Result<(), ()> {
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
		.map_err(|err| {
			warn!("({id}): failed to read `Header::File`: {err:?}");

			// TODO: UI error
			// TODO: Error sent to remote peer
		})?;

	let block_size = BlockSize::from_stream(&mut stream).await.map_err(|err| {
		warn!("({id}): failed to read block size: {err:?}");

		// TODO: UI error
		// TODO: Error sent to remote peer
	})?;
	let size = stream.read_u64_le().await.map_err(|err| {
		warn!("({id}): failed to read file size: {err:?}");

		// TODO: UI error
		// TODO: Error sent to remote peer
	})?;

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
	.await
	.map_err(|err| {
		warn!("({id}): transfer failed: {err:?}");

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;

	Ok(())
}

pub(crate) async fn receiver(
	node: &Arc<Node>,
	HeaderFile {
		id,
		library_id,
		file_path_id,
		range,
	}: HeaderFile,
	mut stream: UnicastStream,
) -> Result<(), ()> {
	#[allow(clippy::panic)] // If you've made it this far that's on you.
	if !node.files_over_p2p_flag.load(Ordering::Relaxed) {
		panic!("Files over P2P is disabled!");
	}

	// TODO: Tunnel and authentication
	// TODO: Use BufReader

	let library = node
		.libraries
		.get_library(&library_id)
		.await
		.ok_or_else(|| {
			warn!("({id}): library not found'{library_id:?}'");

			// TODO: Error in UI
			// TODO: Send error to remote peer??? -> Can we avoid constructing connection until this is done so it's only an error on one side?
		})?;

	let file_path = library
		.db
		.file_path()
		.find_unique(file_path::pub_id::equals(file_path_id.as_bytes().to_vec()))
		.select(file_path_to_handle_p2p_serve_file::select())
		.exec()
		.await
		.map_err(|err| {
			warn!("({id}): error querying for file_path '{file_path_id:?}': {err:?}",);

			// TODO: Error in UI
			// TODO: Send error to remote peer??? -> Can we avoid constructing connection until this is done so it's only an error on one side?
		})?
		.ok_or_else(|| {
			warn!("({id}): file_path not found '{file_path_id:?}'");

			// TODO: Error in UI
			// TODO: Send error to remote peer??? -> Can we avoid constructing connection until this is done so it's only an error on one side?
		})?;

	let location = file_path.location.as_ref().ok_or_else(|| {
		warn!("({id}): file_path '{file_path_id:?} is missing 'location' property");

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;
	let location_path = location.path.as_ref().ok_or_else(|| {
		warn!(
			"({id}): location '{:?} is missing 'path' property",
			location.id
		);

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;
	let path = Path::new(location_path)
			.join(IsolatedFilePathData::try_from((location.id, &file_path)).map_err(|err| {
				warn!("({id}): failed to construct 'IsolatedFilePathData' for location '{:?} '{file_path:?}': {err:?}", location.id);

				// TODO: Error in UI
				// TODO: Send error to remote peer???
			})?);

	debug!("Serving path '{:?}' over P2P", path);

	let file = File::open(&path).await.map_err(|err| {
		warn!("({id}): failed to open file '{path:?}': {err:?}");

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;

	let metadata = file.metadata().await.map_err(|err| {
		warn!("({id}): failed to get metadata for file '{path:?}': {err:?}");

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;
	let block_size = BlockSize::from_size(metadata.len());

	stream
		.write_all(&block_size.to_bytes())
		.await
		.map_err(|err| {
			warn!("({id}): failed to write block size: {err:?}");

			// TODO: Error in UI
			// TODO: Send error to remote peer???
		})?;
	stream
		.write_all(&metadata.len().to_le_bytes())
		.await
		.map_err(|err| {
			warn!("({id}): failed to write length: {err:?}");

			// TODO: Error in UI
			// TODO: Send error to remote peer???
		})?;

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
	.await
	.map_err(|err| {
		warn!("({id}): transfer failed: {err:?}");

		// TODO: Error in UI
		// TODO: Send error to remote peer???
	})?;

	Ok(())
}

// TODO: Unit tests
