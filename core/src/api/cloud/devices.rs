use crate::{api::{Ctx, R}, node::HardwareModel};

use futures::{SinkExt, StreamExt};
use sd_cloud_schema::{
	auth::AccessToken,
	devices::{self, DeviceOS, PubId},
	opaque_ke::{
		rand::rngs::OsRng, ClientLogin, ClientLoginFinishParameters, ClientLoginFinishResult,
		ClientLoginStartResult,
	},
	Client, Service, SpacedriveCipherSuite,
};
use sd_core_cloud_services::QuinnConnection;

use blake3::Hash;
use chrono::DateTime;
use rspc::alpha::AlphaRouter;
use tracing::{debug, error};
use uuid::Uuid;

use super::{handle_comm_error, try_get_cloud_services_client};

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
struct MockDevice {
	pub_id: PubId,
	name: String,
	os: DeviceOS,
	used_storage: u64,
	storage_size: u64,
	created_at: DateTime<chrono::Utc>,
	updated_at: DateTime<chrono::Utc>,
	device_model: HardwareModel,
}

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			// R.query(|node, req: devices::get::Request| async move {
			R.query(|node, _: ()| async move {
				// let devices::get::Response(device) = super::handle_comm_error(
				// 	try_get_cloud_services_client!(node)?
				// 		.devices()
				// 		.get(req)
				// 		.await,
				// 	"Failed to get device;",
				// )??;

				let device = MockDevice {
					name: "Mac Device".to_string(),
					pub_id: PubId(Uuid::now_v7()),
					// Date: 8th Aug 2024 12:00:00 UTC
					created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
						.expect("Failed to parse created_at datetime")
						.with_timezone(&chrono::Utc),
					// Always set to the current time
					updated_at: chrono::Utc::now(),
					os: DeviceOS::MacOS,
					used_storage: 100 * 1024 * 1024 * 1024,
					// Always set to 256 GB in bytes (u64)
					storage_size: 256 * 1024 * 1024 * 1024,
					device_model: HardwareModel::MacBookPro,
				};

				debug!(?device, "Got device");

				Ok(device)
			})
		})
		.procedure("list", {
			R.query(|node, _: ()| async move {
				// let devices::list::Response(devices) = super::handle_comm_error(
				// 	try_get_cloud_services_client!(node)?
				// 		.devices()
				// 		.list(req)
				// 		.await,
				// 	"Failed to list devices;",
				// )??;

				let devices: Vec<MockDevice> = vec![
					MockDevice {
						name: "Mac Device".to_string(),
						pub_id: PubId(Uuid::now_v7()),
						// Date: 8th Aug 2024 12:00:00 UTC
						created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
							.expect("Failed to parse created_at datetime")
							.with_timezone(&chrono::Utc),
						// Always set to the current time
						updated_at: chrono::Utc::now(),
						os: DeviceOS::MacOS,
						// Randomize between 256 GB and 1 TB in bytes (u64)
						storage_size: 256 * 1024 * 1024 * 1024,
						used_storage: 100 * 1024 * 1024 * 1024,
						device_model: HardwareModel::MacMini,
					},
					MockDevice {
						name: "Windows Device".to_string(),
						pub_id: PubId(Uuid::now_v7()),
						// Date: 8th Aug 2024 12:00:00 UTC
						created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
							.expect("Failed to parse created_at datetime")
							.with_timezone(&chrono::Utc),
						// Always set to the current time
						updated_at: chrono::Utc::now(),
						os: DeviceOS::Windows,
						// Randomize between 256 GB and 1 TB in bytes (u64)
						storage_size: 256 * 1024 * 1024 * 1024,
						used_storage: 10 * 1024 * 1024 * 1024,
						device_model: HardwareModel::Other,
					},
					MockDevice {
						name: "Linux Device".to_string(),
						pub_id: PubId(Uuid::now_v7()),
						// Date: 8th Aug 2024 12:00:00 UTC
						created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
							.expect("Failed to parse created_at datetime")
							.with_timezone(&chrono::Utc),
						// Always set to the current time
						updated_at: chrono::Utc::now(),
						os: DeviceOS::Linux,
						// Always set to 256 GB in bytes (u64)
						storage_size: 256 * 1024 * 1024 * 1024,
						used_storage: 50 * 1024 * 1024 * 1024,
						device_model: HardwareModel::Other,
					},
					MockDevice {
						name: "Android Device".to_string(),
						pub_id: PubId(Uuid::now_v7()),
						// Date: 8th Aug 2024 12:00:00 UTC
						created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
							.expect("Failed to parse created_at datetime")
							.with_timezone(&chrono::Utc),
						// Always set to the current time
						updated_at: chrono::Utc::now(),
						os: DeviceOS::Android,
						// Always set to 256 GB in bytes (u64)
						storage_size: 256 * 1024 * 1024 * 1024,
						used_storage: 150 * 1024 * 1024 * 1024,
						device_model: HardwareModel::Android,
					},
					MockDevice {
						name: "iOS Device".to_string(),
						pub_id: PubId(Uuid::now_v7()),
						// Date: 8th Aug 2024 12:00:00 UTC
						created_at: DateTime::parse_from_rfc3339("2024-08-08T12:00:00Z")
							.expect("Failed to parse created_at datetime")
							.with_timezone(&chrono::Utc),
						// Always set to the current time
						updated_at: chrono::Utc::now(),
						os: DeviceOS::IOS,
						// Always set to 256 GB in bytes (u64)
						storage_size: 256 * 1024 * 1024 * 1024,
						used_storage: 200 * 1024 * 1024 * 1024,
						device_model: HardwareModel::IPhone,
					},
				];

				debug!(?devices, "Listed devices");

				Ok(devices)
			})
		})
		.procedure("delete", {
			R.mutation(|node, req: devices::delete::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.devices()
						.delete(req)
						.await,
					"Failed to delete device;",
				)??;

				debug!("Deleted device");

				Ok(())
			})
		})
		.procedure("update", {
			R.mutation(|node, req: devices::update::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.devices()
						.update(req)
						.await,
					"Failed to update device;",
				)??;

				debug!("Updated device");

				Ok(())
			})
		})
}

pub async fn hello(
	client: &Client<QuinnConnection<Service>, Service>,
	access_token: AccessToken,
	device_pub_id: PubId,
	hashed_pub_id: Hash,
) -> Result<(), rspc::Error> {
	use devices::hello::{Request, RequestUpdate, Response, State};

	let ClientLoginStartResult { message, state } = ClientLogin::<SpacedriveCipherSuite>::start(
		&mut OsRng,
		hashed_pub_id.as_bytes().as_slice(),
	)
	.map_err(|e| {
		error!(?e, "OPAQUE error initializing device hello request;");
		rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			"Failed to initialize device login".into(),
		)
	})?;

	let (mut hello_continuation, mut res_stream) = handle_comm_error(
		client
			.devices()
			.hello(Request {
				access_token,
				pub_id: device_pub_id,
				opaque_login_message: Box::new(message),
			})
			.await,
		"Failed to send device hello request;",
	)?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device hello response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	let login_response =
		match handle_comm_error(res, "Communication error on device hello response;")? {
			Ok(Response(State::LoginResponse(login_response))) => login_response,
			Ok(Response(State::End)) => {
				unreachable!("Device hello response MUST not be End here, this is a serious bug and should crash;");
			}
			Err(e) => {
				error!(?e, "Device hello response error;");
				return Err(e.into());
			}
		};

	let ClientLoginFinishResult {
		message,
		export_key,
		..
	} = state
		.finish(
			hashed_pub_id.as_bytes().as_slice(),
			*login_response,
			ClientLoginFinishParameters::default(),
		)
		.map_err(|e| {
			error!(?e, "Device hello finish error;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device login".into(),
			)
		})?;

	hello_continuation
		.send(RequestUpdate {
			opaque_login_finish: Box::new(message),
		})
		.await
		.map_err(|e| {
			error!(?e, "Failed to send device hello request continuation;");
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Failed to finish device login procedure;".into(),
			)
		})?;

	let Some(res) = res_stream.next().await else {
		let message = "Server did not send a device hello END response;";
		error!("{message}");
		return Err(rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			message.to_string(),
		));
	};

	match handle_comm_error(res, "Communication error on device hello response;")? {
		Ok(Response(State::LoginResponse(_))) => {
			unreachable!("Device hello final response MUST be End here, this is a serious bug and should crash;");
		}
		Ok(Response(State::End)) => {}
		Err(e) => {
			error!(?e, "Device hello final response error;");
			return Err(e.into());
		}
	};

	Ok(())
}

pub async fn device_registration() -> Result<(), rspc::Error> {
	Ok(())
}
