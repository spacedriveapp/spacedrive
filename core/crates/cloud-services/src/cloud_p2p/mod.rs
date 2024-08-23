use crate::{CloudServices, Error};

use sd_cloud_schema::{
	cloud_p2p::{authorize_new_device_in_sync_group, CloudP2PALPN, CloudP2PError},
	devices::{self, Device},
	sync::groups::GroupWithLibraryAndDevices,
};
use sd_crypto::{CryptoRng, SeedableRng};

use iroh_base::key::SecretKey as IrohSecretKey;
use iroh_net::{
	discovery::dns::DnsDiscovery,
	relay::{RelayMap, RelayMode, RelayUrl},
	Endpoint, NodeId,
};
use serde::{Deserialize, Serialize};
use tokio::spawn;
use tracing::error;

mod runner;

use runner::Runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
#[repr(transparent)]
#[specta(rename = "CloudP2PTicket")]
pub struct Ticket(u64);

#[derive(Debug, Serialize, specta::Type)]
#[serde(tag = "kind", content = "data")]
pub enum NotifyUser {
	ReceivedJoinSyncGroupRequest {
		ticket: Ticket,
		asking_device: Device,
		sync_group: GroupWithLibraryAndDevices,
	},
	ReceivedJoinSyncGroupResponse {
		response: JoinSyncGroupResponse,
		sync_group: GroupWithLibraryAndDevices,
	},
	SendingJoinSyncGroupResponseError {
		error: JoinSyncGroupError,
		sync_group: GroupWithLibraryAndDevices,
	},
	TimedOutJoinRequest {
		device: Device,
		succeeded: bool,
	},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum JoinSyncGroupError {
	Communication,
	InternalServer,
	Auth,
}

#[derive(Debug, Serialize, specta::Type)]
pub enum JoinSyncGroupResponse {
	Accepted { authorizor_device: Device },
	Failed(CloudP2PError),
	CriticalError,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "data")]
pub enum UserResponse {
	AcceptDeviceInSyncGroup { ticket: Ticket, accepted: bool },
}
#[derive(Debug, Clone)]
pub struct CloudP2P {
	msgs_tx: flume::Sender<runner::Message>,
}

impl CloudP2P {
	pub async fn new(
		current_device_pub_id: devices::PubId,
		cloud_services: &CloudServices,
		mut rng: CryptoRng,
		iroh_secret_key: IrohSecretKey,
		dns_origin_domain: String,
		relay_url: RelayUrl,
	) -> Result<Self, Error> {
		let endpoint = Endpoint::builder()
			.alpns(vec![CloudP2PALPN::LATEST.to_vec()])
			.secret_key(iroh_secret_key)
			.relay_mode(RelayMode::Custom(RelayMap::from_url(relay_url)))
			.discovery(Box::new(DnsDiscovery::new(dns_origin_domain)))
			// Using 0 as port will bind to a random available port chosen by the OS.
			.bind(0)
			.await
			.map_err(Error::CreateCloudP2PEndpoint)?;

		let (msgs_tx, msgs_rx) = flume::bounded(16);

		spawn({
			let runner = Runner::new(current_device_pub_id, cloud_services, endpoint).await?;
			let user_response_rx = cloud_services.user_response_rx.clone();

			async move {
				// All cloned runners share a single state with internal mutability
				while let Err(e) = spawn(runner.clone().run(
					msgs_rx.clone(),
					user_response_rx.clone(),
					CryptoRng::from_seed(rng.generate_fixed()),
				))
				.await
				{
					if e.is_panic() {
						error!("Cloud P2P runner panicked");
					} else {
						break;
					}
				}
			}
		});

		Ok(Self { msgs_tx })
	}

	/// Requests the device with the given connection ID asking for permission to the current device
	/// to join the sync group
	///
	/// # Panics
	/// Will panic if the actor channel is closed, which should never happen
	pub async fn request_join_sync_group(
		&self,
		devices_connection_ids: Vec<NodeId>,
		req: authorize_new_device_in_sync_group::Request,
	) {
		self.msgs_tx
			.send_async(runner::Message::Request(runner::Request::JoinSyncGroup {
				req,
				devices_connection_ids,
			}))
			.await
			.expect("Channel closed");
	}
}

impl Drop for CloudP2P {
	fn drop(&mut self) {
		self.msgs_tx.send(runner::Message::Stop).ok();
	}
}
