#![allow(clippy::panic, clippy::unwrap_used)] // TODO: Finish this

use crate::library::Library;

// use sd_p2p_proto::{decode, encode};
// use sd_sync::CompressedCRDTOperationsPerModelPerDevice;

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};

use super::P2PManager;

mod proto;
pub use proto::*;

pub use originator::run as originator;
mod originator {
	// use crate::p2p::Header;

	use super::*;
	// use responder::tx as rx;
	use sd_core_sync::SyncManager;
	// use sd_p2p_tunnel::Tunnel;

	pub mod tx {

		// use super::*;

		// #[derive(Debug)]
		// pub struct Operations(pub CompressedCRDTOperationsPerModelPerDevice);

		// impl Operations {
		// 	// TODO: Per field errors for better error handling
		// 	pub async fn from_stream(
		// 		stream: &mut (impl AsyncRead + Unpin),
		// 	) -> std::io::Result<Self> {
		// 		Ok(Self(
		// 			rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
		// 		))
		// 	}

		// 	pub fn to_bytes(&self) -> Vec<u8> {
		// 		let Self(args) = self;
		// 		let mut buf = vec![];

		// 		// TODO: Error handling
		// 		encode::buf(&mut buf, &rmp_serde::to_vec_named(&args).unwrap());
		// 		buf
		// 	}
		// }

		// #[cfg(test)]
		// #[tokio::test]
		// async fn test() {
		// 	use sd_sync::CRDTOperation;
		// 	use uuid::Uuid;

		// 	{
		// 		let original = Operations(CompressedCRDTOperationsPerModelPerDevice::new(vec![]));

		// 		let mut cursor = std::io::Cursor::new(original.to_bytes());
		// 		let result = Operations::from_stream(&mut cursor).await.unwrap();
		// 		assert_eq!(original, result);
		// 	}

		// 	{
		// 		let original = Operations(CompressedCRDTOperationsPerModelPerDevice::new(vec![
		// 			CRDTOperation {
		// 				device_pub_id: Uuid::new_v4(),
		// 				timestamp: sync::NTP64(0),
		// 				record_id: rmpv::Value::Nil,
		// 				model_id: 0,
		// 				data: sd_sync::CRDTOperationData::create(),
		// 			},
		// 		]));

		// 		let mut cursor = std::io::Cursor::new(original.to_bytes());
		// 		let result = Operations::from_stream(&mut cursor).await.unwrap();
		// 		assert_eq!(original, result);
		// 	}
		// }
	}

	// #[instrument(skip(sync, p2p))]
	/// REMEMBER: This only syncs one direction!
	pub async fn run(_library: Arc<Library>, _sync: &SyncManager, _p2p: &Arc<super::P2PManager>) {
		// for (remote_identity, peer) in p2p.get_library_instances(&library.id) {
		// 	if !peer.is_connected() {
		// 		continue;
		// 	};

		// 	let sync = sync.clone();

		// 	let library = library.clone();
		// 	tokio::spawn(async move {
		// 		debug!(
		// 			?remote_identity,
		// 			%library.id,
		// 			"Alerting peer of new sync events for library;"
		// 		);

		// 		let mut stream = peer.new_stream().await.unwrap();

		// 		stream.write_all(&Header::Sync.to_bytes()).await.unwrap();

		// 		let mut tunnel = Tunnel::initiator(stream, &library.identity).await.unwrap();

		// 		tunnel
		// 			.write_all(&SyncMessage::NewOperations.to_bytes())
		// 			.await
		// 			.unwrap();
		// 		tunnel.flush().await.unwrap();

		// 		while let Ok(rx::MainRequest::GetOperations(GetOpsArgs {
		// 			timestamp_per_device,
		// 			count,
		// 		})) = rx::MainRequest::from_stream(&mut tunnel).await
		// 		{
		// 			tunnel
		// 				.write_all(
		// 					&tx::Operations(CompressedCRDTOperationsPerModelPerDevice::new(
		// 						sync.get_ops(count, timestamp_per_device).await.unwrap(),
		// 					))
		// 					.to_bytes(),
		// 				)
		// 				.await
		// 				.unwrap();
		// 			tunnel.flush().await.unwrap();
		// 		}
		// 	});
		// }
	}
}

pub use responder::run as responder;
mod responder {

	use super::*;
	// use futures::StreamExt;

	// pub mod tx {
	// 	use serde::{Deserialize, Serialize};

	// 	use super::*;

	// 	#[derive(Serialize, Deserialize, PartialEq, Debug)]
	// 	pub enum MainRequest {
	// 		GetOperations(GetOpsArgs),
	// 		Done,
	// 	}

	// 	impl MainRequest {
	// 		// TODO: Per field errors for better error handling
	// 		pub async fn from_stream(
	// 			stream: &mut (impl AsyncRead + Unpin),
	// 		) -> std::io::Result<Self> {
	// 			Ok(
	// 				// TODO: Error handling
	// 				rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
	// 			)
	// 		}

	// 		pub fn to_bytes(&self) -> Vec<u8> {
	// 			let mut buf = vec![];
	// 			// TODO: Error handling
	// 			encode::buf(&mut buf, &rmp_serde::to_vec_named(&self).unwrap());
	// 			buf
	// 		}
	// 	}

	// 	#[cfg(test)]
	// 	#[tokio::test]
	// 	async fn test() {
	// 		{
	// 			let original = MainRequest::GetOperations(GetOpsArgs {
	// 				timestamp_per_device: vec![],
	// 				count: 0,
	// 			});

	// 			let mut cursor = std::io::Cursor::new(original.to_bytes());
	// 			let result = MainRequest::from_stream(&mut cursor).await.unwrap();
	// 			assert_eq!(original, result);
	// 		}

	// 		{
	// 			let original = MainRequest::Done;

	// 			let mut cursor = std::io::Cursor::new(original.to_bytes());
	// 			let result = MainRequest::from_stream(&mut cursor).await.unwrap();
	// 			assert_eq!(original, result);
	// 		}
	// 	}
	// }

	pub async fn run(
		_stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		_library: Arc<Library>,
	) -> Result<(), ()> {
		// use sync::ingest::*;

		// let ingest = &library.sync.ingest;

		// ingest.event_tx.send(Event::Notification).await.unwrap();

		// let mut rx = pin!(ingest.req_rx.clone());

		// while let Some(req) = rx.next().await {
		// 	const OPS_PER_REQUEST: u32 = 1000;

		// 	let timestamps = match req {
		// 		Request::FinishedIngesting => break,
		// 		Request::Messages { timestamps, .. } => timestamps,
		// 	};

		// 	debug!(?timestamps, "Getting ops for timestamps;");

		// 	stream
		// 		.write_all(
		// 			&tx::MainRequest::GetOperations(sync::GetOpsArgs {
		// 				timestamp_per_device: timestamps,
		// 				count: OPS_PER_REQUEST,
		// 			})
		// 			.to_bytes(),
		// 		)
		// 		.await
		// 		.unwrap();
		// 	stream.flush().await.unwrap();

		// 	let rx::Operations(ops) = rx::Operations::from_stream(stream).await.unwrap();

		// 	let (wait_tx, wait_rx) = tokio::sync::oneshot::channel::<()>();

		// 	// FIXME: If there are exactly a multiple of OPS_PER_REQUEST operations,
		// 	// then this will bug, as we sent `has_more` as true, but we don't have
		// 	// more operations to send.

		// 	ingest
		// 		.event_tx
		// 		.send(Event::Messages(MessagesEvent {
		// 			device_pub_id: library.sync.device_pub_id.clone(),
		// 			has_more: ops.len() == OPS_PER_REQUEST as usize,
		// 			messages: ops,
		// 			wait_tx: Some(wait_tx),
		// 		}))
		// 		.await
		// 		.expect("TODO: Handle ingest channel closed, so we don't loose ops");

		// 	wait_rx.await.unwrap()
		// }

		// debug!("Sync responder done");

		// stream
		// 	.write_all(&tx::MainRequest::Done.to_bytes())
		// 	.await
		// 	.unwrap();
		// stream.flush().await.unwrap();

		Ok(())
	}
}
