use crate::{CRDTOperation, CRDTOperationData, DevicePubId, ModelId, RecordId};

use std::mem;

use serde::{Deserialize, Serialize};
use uhlc::NTP64;
use uuid::Uuid;

pub type CompressedCRDTOperationsPerModel =
	Vec<(ModelId, Vec<(RecordId, Vec<CompressedCRDTOperation>)>)>;

/// Stores a bunch of [`CRDTOperation`]s in a more memory-efficient form for sending to the cloud.
#[derive(Serialize, Deserialize, Debug)]
pub struct CompressedCRDTOperationsPerModelPerDevice(
	pub Vec<(DevicePubId, CompressedCRDTOperationsPerModel)>,
);

impl CompressedCRDTOperationsPerModelPerDevice {
	#[must_use]
	pub fn new(ops: Vec<CRDTOperation>) -> Self {
		let mut compressed = vec![];

		let mut ops_iter = ops.into_iter();

		let Some(first) = ops_iter.next() else {
			return Self(vec![]);
		};

		let mut device_pub_id = first.device_pub_id;
		let mut device_messages = vec![];

		let mut model_id = first.model_id;
		let mut model = vec![];

		let mut record_id = first.record_id.clone();
		let mut record = vec![first.into()];

		for op in ops_iter {
			if device_pub_id != op.device_pub_id {
				model.push((
					mem::replace(&mut record_id, op.record_id.clone()),
					mem::take(&mut record),
				));
				device_messages.push((
					mem::replace(&mut model_id, op.model_id),
					mem::take(&mut model),
				));
				compressed.push((
					mem::replace(&mut device_pub_id, op.device_pub_id),
					mem::take(&mut device_messages),
				));
			} else if model_id != op.model_id {
				model.push((
					mem::replace(&mut record_id, op.record_id.clone()),
					mem::take(&mut record),
				));
				device_messages.push((
					mem::replace(&mut model_id, op.model_id),
					mem::take(&mut model),
				));
			} else if record_id != op.record_id {
				model.push((
					mem::replace(&mut record_id, op.record_id.clone()),
					mem::take(&mut record),
				));
			}

			record.push(CompressedCRDTOperation::from(op));
		}

		model.push((record_id, record));
		device_messages.push((model_id, model));
		compressed.push((device_pub_id, device_messages));

		Self(compressed)
	}

	#[must_use]
	pub fn first(&self) -> Option<(Uuid, u16, &rmpv::Value, &CompressedCRDTOperation)> {
		self.0.first().and_then(|(instance, data)| {
			data.first().and_then(|(model, data)| {
				data.first()
					.and_then(|(record, ops)| ops.first().map(|op| (*instance, *model, record, op)))
			})
		})
	}

	#[must_use]
	pub fn last(&self) -> Option<(Uuid, u16, &rmpv::Value, &CompressedCRDTOperation)> {
		self.0.last().and_then(|(instance, data)| {
			data.last().and_then(|(model, data)| {
				data.last()
					.and_then(|(record, ops)| ops.last().map(|op| (*instance, *model, record, op)))
			})
		})
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.0
			.iter()
			.map(|(_, data)| {
				data.iter()
					.map(|(_, data)| data.iter().map(|(_, ops)| ops.len()).sum::<usize>())
					.sum::<usize>()
			})
			.sum::<usize>()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	#[must_use]
	pub fn into_ops(self) -> Vec<CRDTOperation> {
		let mut ops = vec![];

		for (device_pub_id, device_messages) in self.0 {
			for (model_id, model_messages) in device_messages {
				for (record_id, record) in model_messages {
					for op in record {
						ops.push(CRDTOperation {
							device_pub_id,
							model_id,
							record_id: record_id.clone(),
							timestamp: op.timestamp,
							data: op.data,
						});
					}
				}
			}
		}

		ops
	}
}

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct CompressedCRDTOperation {
	pub timestamp: NTP64,
	pub data: CRDTOperationData,
}

impl From<CRDTOperation> for CompressedCRDTOperation {
	fn from(
		CRDTOperation {
			timestamp, data, ..
		}: CRDTOperation,
	) -> Self {
		Self { timestamp, data }
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn compress() {
		let device_pub_id = Uuid::now_v7();

		let uncompressed = vec![
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 0,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 0,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 0,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 1,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 1,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 0,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
			CRDTOperation {
				device_pub_id,
				timestamp: NTP64(0),
				model_id: 0,
				record_id: rmpv::Value::Nil,
				data: CRDTOperationData::create(),
			},
		];

		let CompressedCRDTOperationsPerModelPerDevice(compressed) =
			CompressedCRDTOperationsPerModelPerDevice::new(uncompressed);

		assert_eq!(compressed[0].1[0].0, 0);
		assert_eq!(compressed[0].1[1].0, 1);
		assert_eq!(compressed[0].1[2].0, 0);

		assert_eq!(compressed[0].1[0].1[0].1.len(), 3);
		assert_eq!(compressed[0].1[1].1[0].1.len(), 2);
		assert_eq!(compressed[0].1[2].1[0].1.len(), 2);
	}

	#[test]
	fn into_ops() {
		let compressed = CompressedCRDTOperationsPerModelPerDevice(vec![(
			Uuid::new_v4(),
			vec![
				(
					0,
					vec![(
						rmpv::Value::Nil,
						vec![
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
						],
					)],
				),
				(
					1,
					vec![(
						rmpv::Value::Nil,
						vec![
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
						],
					)],
				),
				(
					0,
					vec![(
						rmpv::Value::Nil,
						vec![
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
							CompressedCRDTOperation {
								timestamp: NTP64(0),
								data: CRDTOperationData::create(),
							},
						],
					)],
				),
			],
		)]);

		let uncompressed = compressed.into_ops();

		assert_eq!(uncompressed.len(), 7);
		assert_eq!(uncompressed[2].model_id, 0);
		assert_eq!(uncompressed[4].model_id, 1);
		assert_eq!(uncompressed[6].model_id, 0);
	}
}
