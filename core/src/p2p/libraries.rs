use std::sync::Arc;

use sd_p2p2::P2P;
use tracing::{error, warn};

use crate::{
	library::{Libraries, Library, LibraryManagerEvent},
	p2p::IdentityOrRemoteIdentity,
};

pub fn start(p2p: Arc<P2P>, libraries: Arc<Libraries>) {
	tokio::spawn(async move {
		if let Err(err) = libraries
			.rx
			.clone()
			.subscribe(|msg| async move {
				match msg {
					LibraryManagerEvent::InstancesModified(library)
					| LibraryManagerEvent::Load(library) => on_load(&p2p, &library).await,
					LibraryManagerEvent::Edit(library) => {
						// TODO: Send changes to all connected nodes!
						// TODO: Update mdns
					}
					LibraryManagerEvent::Delete(library) => {
						// TODO: Remove library
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `LibraryServices::start` manager aborted with error: {err:?}");
		}
	});
}

async fn on_load(p2p: &P2P, library: &Library) {
	let identities = match library.db.instance().find_many(vec![]).exec().await {
		Ok(library) => library
			.into_iter()
			.filter_map(
				// TODO: Error handling
				|i| match IdentityOrRemoteIdentity::from_bytes(&i.identity) {
					Err(err) => {
						warn!("error parsing identity: {err:?}");
						None
					}
					Ok(IdentityOrRemoteIdentity::Identity(_)) => None,
					Ok(IdentityOrRemoteIdentity::RemoteIdentity(identity)) => Some(identity),
				},
			)
			.collect::<Vec<_>>(),
		Err(err) => {
			warn!("error loading library '{}': {err:?}", library.id);
			return;
		}
	};

	let mut service = p2p.metadata_mut();
	service.insert(
		library.id.to_string(),
		library.identity.to_remote_identity(),
	);
}
