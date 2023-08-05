use std::str::FromStr;

use chrono::{DateTime, Utc};
use sd_p2p::{
	proto::{decode, encode},
	spacetunnel::RemoteIdentity,
};
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use crate::node::Platform;

use super::ModelData;

/// Terminology:
/// Instance - DB model which represents a single `.db` file.
/// Originator - begins the pairing process and is asking to join a library that will be selected by the responder.
/// Responder - is in-charge of accepting or rejecting the originator's request and then selecting which library to "share".

/// A modified version of `prisma::instance::Data` that uses proper validated types for the fields.
#[derive(Debug, PartialEq)]
pub struct Instance {
	pub id: Uuid,
	pub identity: RemoteIdentity,
	pub node_id: Uuid,
	pub node_name: String,
	pub node_platform: Platform,
	pub last_seen: DateTime<Utc>,
	pub date_created: DateTime<Utc>,
}

/// 1. Request for pairing to a library that is owned and will be selected by the responder.
/// Sent `Originator` -> `Responder`.
#[derive(Debug, PartialEq)]
pub struct PairingRequest(/* Originator's instance */ pub Instance);

/// 2. Decision for whether pairing was accepted or rejected once a library is decided on by the user.
/// Sent `Responder` -> `Originator`.
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
pub enum PairingConfirmation {
	Ok,
	Error,
}

/// 4. Sync the data in the database with the originator.
/// Sent `Responder` -> `Originator`.
#[derive(Debug, PartialEq)]
pub enum SyncData {
	Data {
		/// Only included in first request and is an **estimate** of how many models will be sent.
		/// It will likely be wrong so should be constrained to being used for UI purposes only.
		total_models: Option<i64>,
		data: ModelData,
	},
	Finished,
}

impl Instance {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		Ok(Self {
			id: decode::uuid(stream).await.map_err(|e| ("id", e))?,
			identity: RemoteIdentity::from_bytes(
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
			0 => Ok(Self::Accepted {
				library_id: decode::uuid(stream).await.map_err(|e| ("library_id", e))?,
				library_name: decode::string(stream)
					.await
					.map_err(|e| ("library_name", e))?,
				library_description: match decode::string(stream)
					.await
					.map_err(|e| ("library_description", e))?
				{
					s if s.is_empty() => None,
					s => Some(s),
				},
				instances: {
					let len = stream.read_u16_le().await.unwrap();
					let mut instances = Vec::with_capacity(len as usize); // TODO: Prevent DOS

					for _ in 0..len {
						instances.push(Instance::from_stream(stream).await.unwrap());
					}

					instances
				},
			}),
			1 => Ok(Self::Rejected),
			_ => todo!(),
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
				buf.extend((instances.len() as u16).to_le_bytes());
				for instance in instances {
					buf.extend(instance.to_bytes());
				}

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
			_ => todo!(), // TODO: Error handling
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Ok => vec![0],
			Self::Error => vec![1],
		}
	}
}

impl SyncData {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(|e| ("discriminator", e.into()))?;

		match discriminator {
			0 => Ok(Self::Data {
				total_models: match stream
					.read_i64_le()
					.await
					.map_err(|e| ("total_models", e.into()))?
				{
					0 => None,
					n => Some(n),
				},
				data: rmp_serde::from_slice(&decode::buf(stream).await.map_err(|e| ("data", e))?)
					.unwrap(), // TODO: Error handling
			}),
			1 => Ok(Self::Finished),
			_ => todo!(), // TODO: Error handling
		}
	}

	pub fn to_bytes(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
		let mut buf = Vec::new();
		match self {
			Self::Data { total_models, data } => {
				buf.push(0);
				buf.extend((total_models.unwrap_or(0) as i64).to_le_bytes());
				encode::buf(&mut buf, &rmp_serde::to_vec_named(data)?);
			}
			Self::Finished => {
				buf.push(1);
			}
		}

		Ok(buf)
	}
}

#[cfg(test)]
mod tests {
	use sd_p2p::spacetunnel::Identity;

	use super::*;

	#[tokio::test]
	async fn test_types() {
		let identity = Identity::new();
		let instance = || Instance {
			id: Uuid::new_v4(),
			identity: identity.to_remote_identity(),
			node_id: Uuid::new_v4(),
			node_name: "Node Name".into(),
			node_platform: Platform::current(),
			last_seen: Utc::now().into(),
			date_created: Utc::now().into(),
		};

		{
			let original = PairingRequest(instance());

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingRequest::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = PairingResponse::Accepted {
				library_id: Uuid::new_v4(),
				library_name: "Library Name".into(),
				library_description: Some("Library Description".into()),
				instances: vec![instance(), instance(), instance()],
			};

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingResponse::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = PairingResponse::Accepted {
				library_id: Uuid::new_v4(),
				library_name: "Library Name".into(),
				library_description: None,
				instances: vec![],
			};

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingResponse::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = PairingResponse::Rejected;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingResponse::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = PairingConfirmation::Ok;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingConfirmation::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = PairingConfirmation::Error;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = PairingConfirmation::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncData::Data {
				total_models: Some(123),
				data: ModelData::Location(vec![]),
			};

			let mut cursor = std::io::Cursor::new(original.to_bytes().unwrap());
			let result = SyncData::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncData::Finished;

			let mut cursor = std::io::Cursor::new(original.to_bytes().unwrap());
			let result = SyncData::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}
	}
}
