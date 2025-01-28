use anyhow::Context;
use quic_rpc::{server::RpcChannel, Listener};

use crate::schema;

use super::Server;

mod cloud_services;

pub async fn handle(
	server: Server,
	request: schema::Request,
	chan: RpcChannel<schema::Service, impl Listener<schema::Service>>,
) -> anyhow::Result<()> {
	match request {
		schema::Request::CloudServices(req) => cloud_services::router(server, req, chan).await,
	}
	.context("Failed to handle p2p request")
}
