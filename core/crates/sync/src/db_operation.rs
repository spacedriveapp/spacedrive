use sd_prisma::prisma::{cloud_crdt_operation, crdt_operation, instance, PrismaClient};
use sd_sync::CRDTOperation;
use sd_utils::from_bytes_to_uuid;

use tracing::instrument;
use uhlc::NTP64;
use uuid::Uuid;

use super::Error;

crdt_operation::include!(crdt_with_instance {
	instance: select { pub_id }
});

cloud_crdt_operation::include!(cloud_crdt_with_instance {
	instance: select { pub_id }
});

impl crdt_with_instance::Data {
	#[allow(clippy::cast_sign_loss)] // SAFETY: we had to store using i64 due to SQLite limitations
	pub const fn timestamp(&self) -> NTP64 {
		NTP64(self.timestamp as u64)
	}

	pub fn instance(&self) -> Uuid {
		from_bytes_to_uuid(&self.instance.pub_id)
	}

	pub fn into_operation(self) -> Result<CRDTOperation, Error> {
		Ok(CRDTOperation {
			instance: self.instance(),
			timestamp: self.timestamp(),
			record_id: rmp_serde::from_slice(&self.record_id)?,

			model: {
				#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
				// SAFETY: we will not have more than 2^16 models and we had to store using signed
				// integers due to SQLite limitations
				{
					self.model as u16
				}
			},
			data: rmp_serde::from_slice(&self.data)?,
		})
	}
}

impl cloud_crdt_with_instance::Data {
	#[allow(clippy::cast_sign_loss)] // SAFETY: we had to store using i64 due to SQLite limitations
	pub const fn timestamp(&self) -> NTP64 {
		NTP64(self.timestamp as u64)
	}

	pub fn instance(&self) -> Uuid {
		from_bytes_to_uuid(&self.instance.pub_id)
	}

	#[instrument(skip(self), err)]
	pub fn into_operation(self) -> Result<(i32, CRDTOperation), Error> {
		Ok((
			self.id,
			CRDTOperation {
				instance: self.instance(),
				timestamp: self.timestamp(),
				record_id: rmp_serde::from_slice(&self.record_id)?,
				model: {
					#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
					// SAFETY: we will not have more than 2^16 models and we had to store using signed
					// integers due to SQLite limitations
					{
						self.model as u16
					}
				},
				data: rmp_serde::from_slice(&self.data)?,
			},
		))
	}
}

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
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	}
	.to_query(db)
	.select(crdt_operation::select!({ id })) // To don't fetch the whole object for nothing
	.exec()
	.await
	.map_or_else(|e| Err(e.into()), |_| Ok(()))
}
