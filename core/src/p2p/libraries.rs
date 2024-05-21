use std::{
	collections::HashMap,
	sync::{Arc, Mutex, PoisonError},
};

use sd_p2p::{flume::bounded, HookEvent, HookId, PeerConnectionCandidate, RemoteIdentity, P2P};
use tracing::error;

use crate::library::{Libraries, LibraryManagerEvent};

/// A P2P hook which integrates P2P into Spacedrive's library system.
///
/// This hooks is responsible for:
///  - injecting library peers into the P2P system so we can connect to them over internet.
///
pub fn libraries_hook(p2p: Arc<P2P>, libraries: Arc<Libraries>) -> HookId {
	let (tx, rx) = bounded(15);
	let hook_id = p2p.register_hook("sd-libraries-hook", tx);

	let nodes_to_instance = Arc::new(Mutex::new(HashMap::new()));

	let handle = tokio::spawn(async move {
		if let Err(err) = libraries
			.rx
			.clone()
			.subscribe(|msg| {
				let p2p = p2p.clone();
				let nodes_to_instance = nodes_to_instance.clone();
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
									.entry(identity.clone())
									.or_insert(vec![])
									.push(node_identity);

								p2p.clone().discover_peer(
									hook_id,
									node_identity,
									serde_json::from_slice(
										i.metadata.as_ref().expect("this is a required field"),
									)
									.expect("invalid metadata"),
									[PeerConnectionCandidate::Relay].into_iter().collect(),
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
									&i.node_remote_identity
										.as_ref()
										.expect("node remote identity is required"),
								)
								.expect("invalid node remote identity");

								// Skip self
								if i.identity.is_some() {
									continue;
								}

								// Only remove if all instances pointing to this node are removed
								let Some(identities) = nodes_to_instance.get_mut(&identity) else {
									continue;
								};
								identities
									.iter()
									.position(|i| i == &node_identity)
									.map(|i| {
										identities.remove(i);
									});
								if identities.len() == 0 {
									let peers = p2p.peers();
									let Some(peer) = peers.get(&node_identity) else {
										continue;
									};

									peer.undiscover_peer(hook_id);
								}
							}
						}
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `LibraryServices::start` manager aborted with error: {err:?}");
		}
	});

	tokio::spawn(async move {
		while let Ok(event) = rx.recv_async().await {
			match event {
				HookEvent::Shutdown { _guard } => {
					handle.abort();
					break;
				}
				_ => continue,
			}
		}
	});

	hook_id
}
