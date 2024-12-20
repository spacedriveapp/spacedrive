use crate::{
	schema::{
		self,
		cloud_services::{self, authorize_new_device_in_sync_group, notify_new_sync_messages},
	},
	server::Server,
};

use anyhow::Context as _;
use quic_rpc::{server::RpcChannel, Listener};

pub async fn router(
	server: Server,
	request: cloud_services::Request,
	chan: RpcChannel<schema::Service, impl Listener<schema::Service>>,
) -> anyhow::Result<()> {
	match request {
		cloud_services::Request::AuthorizeNewDeviceInSyncGroup(req) => {
			chan.rpc(req, server, authorize_new_device_in_sync_group)
				.await
		}
		cloud_services::Request::NotifyNewSyncMessages(req) => {
			chan.rpc(req, server, notify_new_sync_messages).await
		}
	}
	.context("Failed to handle cloud services request")
}

async fn authorize_new_device_in_sync_group(
	server: Server,
	authorize_new_device_in_sync_group::Request {
		sync_group,
		asking_device,
	}: authorize_new_device_in_sync_group::Request,
) -> authorize_new_device_in_sync_group::Response {
	todo!()
}

async fn notify_new_sync_messages(
	server: Server,
	notify_new_sync_messages::Request {
		sync_group_pub_id,
		device_pub_id,
	}: notify_new_sync_messages::Request,
) -> notify_new_sync_messages::Response {
	todo!()
}
