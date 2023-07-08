use std::str::FromStr;

use chrono::{DateTime, Utc};
use sd_p2p::{
	proto::{decode, encode},
	spacetunnel::Identity,
};
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use crate::node::Platform;

/// Terminology:
/// Instance - DB model which represents a single `.db` file.
/// Originator - begins the pairing process and is asking to join a library that will be selected by the responder.
/// Responder - is in-charge of accepting or rejecting the originator's request and then selecting which library to "share".

/// A modified version of `prisma::instance::Data` that uses proper validated types for the fields.
pub struct Instance {
	pub id: Uuid,
	pub identity: Identity,
	pub node_id: Uuid,
	pub node_name: String,
	pub node_platform: Platform,
	pub last_seen: DateTime<Utc>,
	pub date_created: DateTime<Utc>,
}

/// 1. Request for pairing to a library that is owned and will be selected by the responder.
/// Sent `Originator` -> `Responder`.
pub struct PairingRequest(/* Originator's instance */ pub Instance);

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
		instances: Vec<Instance>,
	},
	// Process will terminate as the user doesn't want to pair
	Rejected,
}

/// 3. Tell the responder that the database was correctly paired.
/// Sent `Originator` -> `Responder`.
pub enum PairingConfirmation {
	Ok,
	Error,
}

impl Instance {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		Ok(Self {
			id: decode::uuid(stream).await.map_err(|e| ("id", e))?,
			identity: Identity::from_bytes(
				&decode::buf(stream).await.map_err(|e| ("identity", e))?,
			)
			.unwrap(), // TODO: Error handling
			node_id: decode::uuid(stream).await.map_err(|e| ("node_id", e))?,
			node_name: decode::string(stream).await.map_err(|e| ("node_name", e))?,
			node_platform: stream
				.read_u8()
				.await
				.map(|b| Platform::try_from(b).unwrap_or(Platform::Unknown))
				.map_err(|e| ("node_platform", e.into()))?,
			last_seen: DateTime::<Utc>::from_str(
				&decode::string(stream).await.map_err(|e| ("last_seen", e))?,
			)
			.unwrap(), // TODO: Error handling
			date_created: DateTime::<Utc>::from_str(
				&decode::string(stream)
					.await
					.map_err(|e| ("date_created", e))?,
			)
			.unwrap(), // TODO: Error handling
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let Self {
			id,
			identity,
			node_id,
			node_name,
			node_platform,
			last_seen,
			date_created,
		} = self;

		let mut buf = Vec::new();

		encode::uuid(&mut buf, id);
		encode::buf(&mut buf, &identity.to_bytes());
		encode::uuid(&mut buf, node_id);
		encode::string(&mut buf, node_name);
		buf.push(*node_platform as u8);
		encode::string(&mut buf, &last_seen.to_string());
		encode::string(&mut buf, &date_created.to_string());

		buf
	}
}

impl PairingRequest {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		Ok(Self(Instance::from_stream(stream).await?))
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let Self(instance) = self;
		Instance::to_bytes(instance)
	}
}

impl PairingResponse {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		// TODO: Error handling
		match stream.read_u8().await.unwrap() {
			0 => {
				todo!();
			}
			1 => {
				todo!();
			}
			_ => {
				todo!();
			}
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Accepted {
				library_id,
				library_name,
				library_description,
				instances,
			} => {
				let mut buf = vec![0];

				encode::uuid(&mut buf, library_id);
				encode::string(&mut buf, library_name);
				encode::string(&mut buf, library_description.as_deref().unwrap_or(""));
				// encode::vec(&mut buf, instances, Instance::to_bytes); // TODO
				todo!();

				buf
			}
			Self::Rejected => vec![1],
		}
	}
}

impl PairingConfirmation {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		// TODO: Error handling
		match stream.read_u8().await.unwrap() {
			0 => Ok(Self::Ok),
			1 => Ok(Self::Error),
			_ => {
				todo!();
			}
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Ok => vec![0],
			Self::Error => vec![1],
		}
	}
}

// TODO: Unit testing
