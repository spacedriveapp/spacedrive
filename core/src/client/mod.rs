use anyhow::Result;
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::infra::daemon::client::DaemonClient;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse};

pub trait Wire {
	const METHOD: &'static str;
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

	pub async fn action<A>(&self, action: &A) -> Result<Vec<u8>>
	where
		A: Wire + Serialize,
	{
		let payload = encode_to_vec(action, standard())?;
		let resp = self
			.daemon
			.send(&DaemonRequest::Action {
				method: A::METHOD.into(),
				payload,
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::Ok(bytes) => Ok(bytes),
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e.to_string())),
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
				method: Q::METHOD.into(),
				payload,
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::Ok(bytes) => Ok(decode_from_slice(&bytes, standard())?.0),
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e.to_string())),
				other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			},
			Err(e) => Err(anyhow::anyhow!(e.to_string())),
		}
	}

	pub async fn send_raw_request(&self, req: &DaemonRequest) -> Result<DaemonResponse> {
		self.daemon
			.send(req)
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))
	}
}
