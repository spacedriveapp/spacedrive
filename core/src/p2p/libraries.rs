use std::{
	collections::HashMap,
	sync::{Arc, Mutex, PoisonError},
};

use sd_p2p::{hooks::QuicHandle, RemoteIdentity, P2P};
use tracing::error;

use crate::library::{Libraries, LibraryManagerEvent};

/// A P2P hook which integrates P2P into Spacedrive's library system.
///
/// This hooks is responsible for:
///  - injecting library peers into the P2P system so we can connect to them over internet.
///
pub fn libraries_hook(p2p: Arc<P2P>, quic: Arc<QuicHandle>, libraries: Arc<Libraries>) {
	let nodes_to_instance = Arc::new(Mutex::new(HashMap::new()));

	let handle = tokio::spawn({
		let quic = quic.clone();

		async move {
			if let Err(e) = libraries
				.rx
				.clone()
				.subscribe(|msg| {
					let p2p = p2p.clone();
					let nodes_to_instance = nodes_to_instance.clone();
					let quic = quic.clone();

					async move {
						match msg {
							LibraryManagerEvent::InstancesModified(library)
							| LibraryManagerEvent::Load(library) => {
								p2p.metadata_mut().insert(
									library.id.to_string(),
									library.identity.to_remote_identity().to_string(),
								);

								let Ok(instances) =
									library.db.instance().find_many(vec![]).exec().await
								else {
									return;
								};

								let mut nodes_to_instance = nodes_to_instance
									.lock()
									.unwrap_or_else(PoisonError::into_inner);

								for i in instances.iter() {
									let identity = RemoteIdentity::from_bytes(&i.remote_identity)
										.expect("invalid instance identity");
									let node_identity = RemoteIdentity::from_bytes(
										i.node_remote_identity
											.as_ref()
											.expect("node remote identity is required"),
									)
									.expect("invalid node remote identity");

									// Skip self
									if i.identity.is_some() {
										continue;
									}

									nodes_to_instance
										.entry(identity)
										.or_insert(vec![])
										.push(node_identity);

									quic.track_peer(
										node_identity,
										serde_json::from_slice(
											i.metadata.as_ref().expect("this is a required field"),
										)
										.expect("invalid metadata"),
									);
								}
							}
							LibraryManagerEvent::Edit(_library) => {
								// TODO: Send changes to all connected nodes or queue sending for when they are online!
							}
							LibraryManagerEvent::Delete(library) => {
								p2p.metadata_mut().remove(&library.id.to_string());

								let Ok(instances) =
									library.db.instance().find_many(vec![]).exec().await
								else {
									return;
								};

								let mut nodes_to_instance = nodes_to_instance
									.lock()
									.unwrap_or_else(PoisonError::into_inner);

								for i in instances.iter() {
									let identity = RemoteIdentity::from_bytes(&i.remote_identity)
										.expect("invalid remote identity");
									let node_identity = RemoteIdentity::from_bytes(
										i.node_remote_identity
											.as_ref()
											.expect("node remote identity is required"),
									)
									.expect("invalid node remote identity");

									// Skip self
									if i.identity.is_some() {
										continue;
									}

									// Only remove if all instances pointing to this node are removed
									let Some(identities) = nodes_to_instance.get_mut(&identity)
									else {
										continue;
									};
									if let Some(i) =
										identities.iter().position(|i| i == &node_identity)
									{
										identities.remove(i);
									}
									if identities.is_empty() {
										quic.untrack_peer(node_identity);
									}
								}
							}
						}
					}
				})
				.await
			{
				error!(?e, "Core may become unstable! `LibraryServices::start` manager aborted with error;");
			}
		}
	});

	tokio::spawn(async move {
		quic.shutdown().await;
		handle.abort();
	});
}
