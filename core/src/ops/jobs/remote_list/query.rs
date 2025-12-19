use super::output::{RemoteJobsAllDevicesOutput, RemoteJobsForDeviceOutput};
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Query for remote jobs on a specific device
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsForDeviceInput {
	pub device_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsForDeviceQuery {
	device_id: Uuid,
}

impl CoreQuery for RemoteJobsForDeviceQuery {
	type Input = RemoteJobsForDeviceInput;
	type Output = RemoteJobsForDeviceOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			device_id: input.device_id,
		})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let remote_cache = &context.remote_job_cache;
		let jobs = remote_cache.get_device_jobs(self.device_id).await;

		Ok(RemoteJobsForDeviceOutput { jobs })
	}
}

crate::register_core_query!(RemoteJobsForDeviceQuery, "jobs.remote.for_device");

/// Query for all remote jobs across all devices
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsAllDevicesInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsAllDevicesQuery {}

impl CoreQuery for RemoteJobsAllDevicesQuery {
	type Input = RemoteJobsAllDevicesInput;
	type Output = RemoteJobsAllDevicesOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self {})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let remote_cache = &context.remote_job_cache;
		let jobs_by_device = remote_cache.get_all_active_jobs().await;

		Ok(RemoteJobsAllDevicesOutput { jobs_by_device })
	}
}

crate::register_core_query!(RemoteJobsAllDevicesQuery, "jobs.remote.all_devices");
