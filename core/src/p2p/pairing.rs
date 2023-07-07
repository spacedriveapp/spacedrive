use sd_p2p::PeerId;
use sd_prisma::prisma::instance;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::{
	library::{Library, LibraryManager},
	node::Platform,
};

/// Terminology:
/// Instance - DB model which represents a single `.db` file.
/// Originator - begins the pairing process and is asking to join a library that will be selected by the responder.
/// Responder - is in-charge of accepting or rejecting the originator's request and then selecting which library to "share".

/// 1. Request for pairing to a library that is owned and will be selected by the responder.
/// Sent `Originator` -> `Responder`.
pub struct PairingRequest {
	// Originator's information
	node_id: Uuid,
	node_name: String,
	node_platform: Platform,
}

/// 2. Decision for whether pairing was accepted or rejected once a library is decided on by the user.
/// Sent `Responder` -> `Originator`.
pub enum PairingResponse {
	/// Pairing was accepted and the responder chose the library of their we are pairing to.
	Accepted {
		// Library information
		library_id: Uuid,
		library_name: String,
		library_description: Option<String>,

		// All instances in the library
		// Copying these means we are instantly paired with everyone else that is already in the library
		// NOTE: It's super important the `identity` field is converted from a private key to a public key before sending!!!
		instances: Vec<instance::Data>,
	},
	// Process will terminate as the user doesn't want to pair
	Rejected,
}

/// 3. The newly created instance that represents the Originator.
/// Sent `Originator` -> `Responder`.
pub struct InsertOriginatorInstance {
	// Originator's instance to be added to the responder's DB
	instance: instance::Data,
}

/// 3. Confirm that the originator's instance was inserted into the responder's DB.
/// Sent `Responder` -> `Originator`.
pub enum ConfirmInsertOriginatorInstance {
	Ok,
	Error,
}

pub async fn originator(peer_id: PeerId) {
	info!("Beginning pairing as originator to remote peer '{peer_id}'");

	// send(PairingRequest {}).await;

	todo!();
}

pub async fn responder(peer_id: PeerId, library_manager: &LibraryManager) {
	info!("Beginning pairing as responder to remote peer '{peer_id}'");

	let msg: PairingRequest = todo!(); // Receive from network

	// Prompt the user
	let PairingDecision::Accept(decision) = todo!() else {
		// info!();

		// send(PairingResponse::Rejected);

		return;
	};

	let library: &Library = todo!();
	let instances: Vec<instance::Data> = todo!().into_iter().map(|i| {
		// TODO: If `i.identity` contains a public/private keypair replace it with the public key

		i
	});

	// send(PairingResponse::Accepted {
	// 	library_id: library.config.id,
	// 	library_name: library.config.name,
	// 	library_description: library.config.description,
	//  instances,
	// });

	let msg: InsertOriginatorInstance = todo!(); // Receive from network

	// library.db.instance().create(msg.instance).await?;

	// send(ConfirmInsertOriginatorInstance::Ok);
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub enum PairingDecision {
	Accept(Uuid),
	Reject,
}

// TODO: Unit tests
