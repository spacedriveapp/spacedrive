use anyhow::Result;
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
	pub fn new(socket: PathBuf) -> Self { Self { daemon: DaemonClient::new(socket) } }

	pub async fn action<A>(&self, action: &A) -> Result<()>
	where
		A: Wire + Serialize,
	{
		let payload = bincode::serialize(action)?;
		match self
			.daemon
			.send(&DaemonRequest::Action { type_id: A::TYPE_ID.into(), payload })
			.await?
		{
			DaemonResponse::Ok(_) => Ok(()),
			DaemonResponse::Error(e) => Err(anyhow::anyhow!(e)),
			other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
		}
	}

	pub async fn query<Q, O>(&self, query: &Q) -> Result<O>
	where
		Q: Wire + Serialize,
		O: DeserializeOwned,
	{
		let payload = bincode::serialize(query)?;
		match self
			.daemon
			.send(&DaemonRequest::Query { type_id: Q::TYPE_ID.into(), payload })
			.await?
		{
			DaemonResponse::Ok(bytes) => Ok(bincode::deserialize(&bytes)?),
			DaemonResponse::Error(e) => Err(anyhow::anyhow!(e)),
			other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
		}
	}
}


