use crate::{
	api::{Ctx, R},
	try_get_cloud_services_client,
};

use chrono::DateTime;
use sd_cloud_schema::devices::{self, DeviceOS, PubId};

use rspc::alpha::AlphaRouter;
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
struct MockDevice {
	pub_id: PubId,
	name: String,
	os: DeviceOS,
	storage_size: u64,
	created_at: DateTime<chrono::Utc>,
	updated_at: DateTime<chrono::Utc>,
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
					// Always set to 256 GB in bytes (u64)
					storage_size: 256 * 1024 * 1024 * 1024,
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
					},
				];

				debug!(?devices, "Listed devices");

				Ok(devices)
			})
		})
		.procedure("delete", {
			R.mutation(|node, req: devices::delete::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
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
					try_get_cloud_services_client!(node)?
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
