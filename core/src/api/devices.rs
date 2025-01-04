use crate::node::HardwareModel;
use rspc::alpha::AlphaRouter;
use sd_cloud_schema::devices::DeviceOS;
use sd_core_prisma_helpers::DevicePubId;
use sd_prisma::prisma::device;
use serde::Serialize;
use specta::Type;

use super::{utils::library, Ctx, R};

#[derive(Type, Serialize, Clone, Debug)]
pub struct Device {
	pub id: i32,
	pub pub_id: DevicePubId,
	pub name: String,
	pub os: DeviceOS,
	pub hardware_model: HardwareModel,
	pub date_created: chrono::DateTime<chrono::FixedOffset>,

	pub is_current_device: bool,
}

impl From<(device::Data, &DevicePubId)> for Device {
	fn from((d, current_device_pub_id): (device::Data, &DevicePubId)) -> Self {
		let pub_id = DevicePubId::from(d.pub_id);

		Self {
			id: d.id,
			is_current_device: pub_id == *current_device_pub_id,
			pub_id,
			name: d.name.unwrap_or_default(),
			os: d
				.os
				.expect("is not actually optional")
				.try_into()
				.expect("is not actually optional"),
			hardware_model: d
				.hardware_model
				.expect("is not actually optional")
				.try_into()
				.expect("is not actually optional"),
			date_created: d.date_created.expect("is not actually optional"),
		}
	}
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure(
		"list",
		R.with2(library())
			.query(|(node, library), _: ()| async move {
				let current_device_pub_id = node.config.get().await.id;
				Ok(library
					.db
					.device()
					.find_many(vec![])
					.exec()
					.await?
					.into_iter()
					.map(|d| Device::from((d, &current_device_pub_id)))
					.collect::<Vec<_>>())
			}),
	)
}
