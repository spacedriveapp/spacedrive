use anyhow::Result;
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::infra::daemon::client::DaemonClient;
use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};

pub trait Wire {
	const TYPE_ID: &'static str;
}

pub struct CoreClient {
	daemon: DaemonClient,
}

impl CoreClient {
	pub fn new(socket: PathBuf) -> Self {
		Self {
			daemon: DaemonClient::new(socket),
		}
	}

	pub async fn action<A>(&self, action: &A) -> Result<()>
	where
		A: Wire + Serialize,
	{
		let payload = encode_to_vec(action, standard())?;
		let resp = self
			.daemon
			.send(&DaemonRequest::Action {
				type_id: A::TYPE_ID.into(),
				payload,
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::Ok(_) => Ok(()),
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e)),
				other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			},
			Err(e) => Err(anyhow::anyhow!(e.to_string())),
		}
	}

	pub async fn query<Q, O>(&self, query: &Q) -> Result<O>
	where
		Q: Wire + Serialize,
		O: DeserializeOwned,
	{
		let payload = encode_to_vec(query, standard())?;
		let resp = self
			.daemon
			.send(&DaemonRequest::Query {
				type_id: Q::TYPE_ID.into(),
				payload,
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::Ok(bytes) => Ok(decode_from_slice(&bytes, standard())?.0),
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e)),
				other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			},
			Err(e) => Err(anyhow::anyhow!(e.to_string())),
		}
	}
}
