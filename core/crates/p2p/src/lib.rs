use sd_core_cloud_services::CloudServices;

use sd_cloud_schema::devices;
use sd_crypto::CryptoRng;

use std::{path::Path, sync::Arc};

use iroh::{
	discovery::{
		dns::DnsDiscovery, local_swarm_discovery::LocalSwarmDiscovery, pkarr::dht::DhtDiscovery,
		ConcurrentDiscovery,
	},
	key::SecretKey,
	Endpoint, NodeId, RelayMap, RelayMode, RelayUrl,
};
use quic_rpc::{server::IrohListener, RpcServer};
use tokio::{
	fs, io,
	sync::{oneshot, RwLock},
};
use url::Url;

mod error;
mod schema;
mod server;

use server::Server;

pub use error::Error;

const KNOWN_DEVICES_FILE_NAME: &str = "known_devices.bin";

#[derive(Debug, Clone)]
pub struct P2P {
	current_device_pub_id: devices::PubId,
	known_devices_file_path: Arc<Box<Path>>,
	endpoint: Endpoint,
	cloud_services: Arc<RwLock<Option<CloudServices>>>,
	known_devices: Arc<RwLock<Vec<NodeId>>>,
	cancel_tx: flume::Sender<oneshot::Sender<()>>,
}

impl P2P {
	pub async fn new(
		data_directory: impl AsRef<Path> + Send,
		current_device_pub_id: devices::PubId,
		rng: CryptoRng,
		iroh_secret_key: SecretKey,
		dns_origin_domain: String,
		dns_pkarr_url: Url,
		relay_url: RelayUrl,
	) -> Result<Self, Error> {
		async fn inner(
			data_directory: &Path,
			current_device_pub_id: devices::PubId,
			rng: CryptoRng,
			iroh_secret_key: SecretKey,
			dns_origin_domain: String,
			dns_pkarr_url: Url,
			relay_url: RelayUrl,
		) -> Result<P2P, Error> {
			let endpoint = Endpoint::builder()
				.alpns(vec![schema::ALPN::LATEST.to_vec()])
				.discovery(Box::new(ConcurrentDiscovery::from_services(vec![
					Box::new(DnsDiscovery::new(dns_origin_domain)),
					Box::new(
						LocalSwarmDiscovery::new(iroh_secret_key.public())
							.map_err(Error::LocalSwarmDiscoveryInit)?,
					),
					Box::new(
						DhtDiscovery::builder()
							.secret_key(iroh_secret_key.clone())
							.pkarr_relay(dns_pkarr_url)
							.build()
							.map_err(Error::DhtDiscoveryInit)?,
					),
				])))
				.secret_key(iroh_secret_key)
				.relay_mode(RelayMode::Custom(RelayMap::from_url(relay_url)))
				.bind()
				.await
				.map_err(Error::SetupEndpoint)?;

			let (cancel_tx, cancel_rx) = flume::bounded(1);

			let known_devices_file_path = data_directory
				.join(KNOWN_DEVICES_FILE_NAME)
				.into_boxed_path();

			let known_devices = Arc::new(RwLock::new(
				P2P::load_known_devices(&known_devices_file_path).await?,
			));

			let cloud_services = Arc::default();

			Server::new(
				current_device_pub_id,
				Arc::clone(&cloud_services),
				Arc::clone(&known_devices),
			)
			.dispatch(
				RpcServer::new(
					IrohListener::<schema::Service>::new(endpoint.clone())
						.map_err(Error::SetupListener)?,
				),
				cancel_rx,
			);

			Ok(P2P {
				current_device_pub_id,
				endpoint,
				cloud_services,
				known_devices,
				known_devices_file_path: Arc::new(known_devices_file_path),
				cancel_tx,
			})
		}

		inner(
			data_directory.as_ref(),
			current_device_pub_id,
			rng,
			iroh_secret_key,
			dns_origin_domain,
			dns_pkarr_url,
			relay_url,
		)
		.await
	}

	async fn load_known_devices(
		known_devices_file_path: impl AsRef<Path> + Send,
	) -> Result<Vec<NodeId>, Error> {
		async fn inner(known_devices_file_path: &Path) -> Result<Vec<NodeId>, Error> {
			match fs::read(known_devices_file_path).await {
				Ok(data) => postcard::from_bytes(&data).map_err(Error::DeserializeKnownDevices),
				Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
				Err(e) => Err(Error::LoadKnownDevices(e)),
			}
		}

		inner(known_devices_file_path.as_ref()).await
	}

	pub async fn set_cloud_services(&self, cloud_services: CloudServices) {
		self.cloud_services.write().await.replace(cloud_services);
	}

	pub async fn shutdown(&self) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();
		self.cancel_tx.send_async(tx).await.unwrap();
		rx.await.unwrap();

		fs::write(
			self.known_devices_file_path.as_ref(),
			&postcard::to_stdvec(&*self.known_devices.read().await)
				.map_err(Error::SerializeKnownDevices)?,
		)
		.await
		.map_err(Error::SaveKnownDevices)
	}
}
