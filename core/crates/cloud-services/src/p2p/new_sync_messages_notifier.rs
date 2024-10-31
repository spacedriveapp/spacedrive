use crate::{token_refresher::TokenRefresher, Error};

use sd_cloud_schema::{
	cloud_p2p::{Client, CloudP2PALPN, Service},
	devices,
	sync::groups,
};

use std::time::Duration;

use futures_concurrency::future::Join;
use iroh_net::{Endpoint, NodeId};
use quic_rpc::{transport::quinn::QuinnConnection, RpcClient};
use tokio::time::Instant;
use tracing::{debug, error, instrument, warn};

use super::runner::Message;

const CACHED_MAX_DURATION: Duration = Duration::from_secs(60 * 5);

pub async fn dispatch_notifier(
	group_pub_id: groups::PubId,
	device_pub_id: devices::PubId,
	devices: Option<(Instant, Vec<(devices::PubId, NodeId)>)>,
	msgs_tx: flume::Sender<Message>,
	cloud_services: sd_cloud_schema::Client<
		QuinnConnection<sd_cloud_schema::Service>,
		sd_cloud_schema::Service,
	>,
	token_refresher: TokenRefresher,
	endpoint: Endpoint,
) {
	match notify_peers(
		group_pub_id,
		device_pub_id,
		devices,
		cloud_services,
		token_refresher,
		endpoint,
	)
	.await
	{
		Ok((true, devices)) => {
			if msgs_tx
				.send_async(Message::UpdateCachedDevices((group_pub_id, devices)))
				.await
				.is_err()
			{
				warn!("Failed to send update cached devices message to update cached devices");
			}
		}

		Ok((false, _)) => {}

		Err(e) => {
			error!(?e, "Failed to notify peers");
		}
	}
}

#[instrument(skip(cloud_services, token_refresher, endpoint))]
async fn notify_peers(
	group_pub_id: groups::PubId,
	device_pub_id: devices::PubId,
	devices: Option<(Instant, Vec<(devices::PubId, NodeId)>)>,
	cloud_services: sd_cloud_schema::Client<
		QuinnConnection<sd_cloud_schema::Service>,
		sd_cloud_schema::Service,
	>,
	token_refresher: TokenRefresher,
	endpoint: Endpoint,
) -> Result<(bool, Vec<(devices::PubId, NodeId)>), Error> {
	let (devices, update_cache) = match devices {
		Some((when, devices)) if when.elapsed() < CACHED_MAX_DURATION => (devices, false),
		_ => {
			debug!("Fetching devices connection ids for group");
			let groups::get::Response(groups::get::ResponseKind::DevicesConnectionIds(devices)) =
				cloud_services
					.sync()
					.groups()
					.get(groups::get::Request {
						access_token: token_refresher.get_access_token().await?,
						pub_id: group_pub_id,
						kind: groups::get::RequestKind::DevicesConnectionIds,
					})
					.await??
			else {
				unreachable!("Only DevicesConnectionIds response is expected, as we requested it");
			};

			(devices, true)
		}
	};

	send_notifications(group_pub_id, device_pub_id, &devices, &endpoint).await;

	Ok((update_cache, devices))
}

async fn send_notifications(
	group_pub_id: groups::PubId,
	device_pub_id: devices::PubId,
	devices: &[(devices::PubId, NodeId)],
	endpoint: &Endpoint,
) {
	devices
		.iter()
		.filter(|(peer_device_pub_id, _)| *peer_device_pub_id != device_pub_id)
		.map(|(peer_device_pub_id, connection_id)| async move {
			if let Err(e) =
				connect_and_send_notification(group_pub_id, device_pub_id, connection_id, endpoint)
					.await
			{
				// Using just a debug log here because we don't want to spam the logs with
				// every single notification failure, as this is more a nice to have feature than a
				// critical one
				debug!(?e, %peer_device_pub_id, "Failed to send new sync messages notification to peer");
			} else {
				debug!(%peer_device_pub_id, "Sent new sync messages notification to peer");
			}
		})
		.collect::<Vec<_>>()
		.join()
		.await;
}

async fn connect_and_send_notification(
	group_pub_id: groups::PubId,
	device_pub_id: devices::PubId,
	connection_id: &NodeId,
	endpoint: &Endpoint,
) -> Result<(), Error> {
	let client = Client::new(RpcClient::new(QuinnConnection::<Service>::from_connection(
		endpoint
			.connect(*connection_id, CloudP2PALPN::LATEST)
			.await
			.map_err(Error::ConnectToCloudP2PNode)?,
	)));

	if let Err(e) = client
		.notify_new_sync_messages(
			sd_cloud_schema::cloud_p2p::notify_new_sync_messages::Request {
				sync_group_pub_id: group_pub_id,
				device_pub_id,
			},
		)
		.await?
	{
		warn!(
			?e,
			"This route shouldn't return an error, it's just a notification",
		);
	};

	Ok(())
}
