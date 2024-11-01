use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::prisma::{cloud_crdt_operation, crdt_operation, PrismaClient};
use sd_sync::CRDTOperation;
use sd_utils::uuid_to_bytes;

use tracing::instrument;
use uhlc::NTP64;

use super::Error;

#[instrument(skip(op, db), err)]
pub async fn write_crdt_op_to_db(op: &CRDTOperation, db: &PrismaClient) -> Result<(), Error> {
	crdt_operation::Create {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we have to store using i64 due to SQLite limitations
			{
				op.timestamp.0 as i64
			}
		},
		device_pub_id: uuid_to_bytes(&op.device_pub_id),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	}
	.to_query(db)
	.select(crdt_operation::select!({ id })) // To don't fetch the whole object for nothing
	.exec()
	.await?;

	Ok(())
}

pub fn from_crdt_ops(
	crdt_operation::Data {
		timestamp,
		model,
		record_id,
		data,
		device_pub_id,
		..
	}: crdt_operation::Data,
) -> Result<CRDTOperation, Error> {
	Ok(CRDTOperation {
		device_pub_id: DevicePubId::from(device_pub_id).into(),
		timestamp: {
			#[allow(clippy::cast_sign_loss)]
			{
				// SAFETY: we had to store using i64 due to SQLite limitations
				NTP64(timestamp as u64)
			}
		},
		model_id: {
			#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
			{
				// SAFETY: we will not have more than 2^16 models and we had to store using signed
				// integers due to SQLite limitations
				model as u16
			}
		},
		record_id: rmp_serde::from_slice(&record_id)?,
		data: rmp_serde::from_slice(&data)?,
	})
}

pub fn from_cloud_crdt_ops(
	cloud_crdt_operation::Data {
		id,
		timestamp,
		model,
		record_id,
		data,
		device_pub_id,
		..
	}: cloud_crdt_operation::Data,
) -> Result<(cloud_crdt_operation::id::Type, CRDTOperation), Error> {
	Ok((
		id,
		CRDTOperation {
			device_pub_id: DevicePubId::from(device_pub_id).into(),
			timestamp: {
				#[allow(clippy::cast_sign_loss)]
				{
					// SAFETY: we had to store using i64 due to SQLite limitations
					NTP64(timestamp as u64)
				}
			},
			model_id: {
				#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
				{
					// SAFETY: we will not have more than 2^16 models and we had to store using signed
					// integers due to SQLite limitations
					model as u16
				}
			},
			record_id: rmp_serde::from_slice(&record_id)?,
			data: rmp_serde::from_slice(&data)?,
		},
	))
}
