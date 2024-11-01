use crate::{CRDTOperation, CRDTOperationData, DevicePubId, ModelId, RecordId};

use std::collections::{hash_map::Entry, BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use uhlc::NTP64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompressedCRDTOperationsPerModel(pub Vec<(ModelId, CompressedCRDTOperationsPerRecord)>);

pub type CompressedCRDTOperationsPerRecord = Vec<(RecordId, Vec<CompressedCRDTOperation>)>;

/// Stores a bunch of [`CRDTOperation`]s in a more memory-efficient form for sending to the cloud.
#[derive(Serialize, Deserialize, Debug)]
pub struct CompressedCRDTOperationsPerModelPerDevice(
	pub Vec<(DevicePubId, CompressedCRDTOperationsPerModel)>,
);

impl CompressedCRDTOperationsPerModelPerDevice {
	/// Creates a new [`CompressedCRDTOperationsPerModelPerDevice`] from a vector of [`CRDTOperation`]s.
	///
	/// # Panics
	///
	/// Will panic if for some reason `rmp_serde::to_vec` fails to serialize a `rmpv::Value` to bytes.
	#[must_use]
	pub fn new(ops: Vec<CRDTOperation>) -> Self {
		let mut compressed_map = BTreeMap::<
			DevicePubId,
			BTreeMap<ModelId, HashMap<Vec<u8>, (RecordId, Vec<CompressedCRDTOperation>)>>,
		>::new();

		for CRDTOperation {
			device_pub_id,
			timestamp,
			model_id,
			record_id,
			data,
		} in ops
		{
			let records = compressed_map
				.entry(device_pub_id)
				.or_default()
				.entry(model_id)
				.or_default();

			// Can't use RecordId as a key because rmpv::Value doesn't implement Hash + Eq.
			// So we use it's serialized bytes as a key.
			let record_id_bytes =
				rmp_serde::to_vec_named(&record_id).expect("already serialized to Value");

			match records.entry(record_id_bytes) {
				Entry::Occupied(mut entry) => {
					entry
						.get_mut()
						.1
						.push(CompressedCRDTOperation { timestamp, data });
				}
				Entry::Vacant(entry) => {
					entry.insert((record_id, vec![CompressedCRDTOperation { timestamp, data }]));
				}
			}
		}

		Self(
			compressed_map
				.into_iter()
				.map(|(device_pub_id, model_map)| {
					(
						device_pub_id,
						CompressedCRDTOperationsPerModel(
							model_map
								.into_iter()
								.map(|(model_id, ops_per_record_map)| {
									(model_id, ops_per_record_map.into_values().collect())
								})
								.collect(),
						),
					)
				})
				.collect(),
		)
	}

	/// Creates a new [`CompressedCRDTOperationsPerModel`] from crdt operation of a single device.
	///
	/// # Panics
	/// Will panic if there are more than one device.
	#[must_use]
	pub fn new_single_device(
		ops: Vec<CRDTOperation>,
	) -> (DevicePubId, CompressedCRDTOperationsPerModel) {
		let Self(mut compressed) = Self::new(ops);

		assert_eq!(compressed.len(), 1, "Expected a single device");

		compressed.remove(0)
	}

	#[must_use]
	pub fn first(&self) -> Option<(DevicePubId, ModelId, &RecordId, &CompressedCRDTOperation)> {
		self.0.first().and_then(|(instance, data)| {
			data.0.first().and_then(|(model, data)| {
				data.first()
					.and_then(|(record, ops)| ops.first().map(|op| (*instance, *model, record, op)))
			})
		})
	}

	#[must_use]
	pub fn last(&self) -> Option<(DevicePubId, ModelId, &RecordId, &CompressedCRDTOperation)> {
		self.0.last().and_then(|(instance, data)| {
			data.0.last().and_then(|(model, data)| {
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
				data.0
					.iter()
					.map(|(_, data)| data.iter().map(|(_, ops)| ops.len()).sum::<usize>())
					.sum::<usize>()
			})
			.sum()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	#[must_use]
	pub fn into_ops(self) -> Vec<CRDTOperation> {
		let mut ops = Vec::with_capacity(self.len());

		for (device_pub_id, device_messages) in self.0 {
			for (model_id, model_messages) in device_messages.0 {
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

impl CompressedCRDTOperationsPerModel {
	#[must_use]
	pub fn first(&self) -> Option<(ModelId, &RecordId, &CompressedCRDTOperation)> {
		self.0.first().and_then(|(model_id, data)| {
			data.first()
				.and_then(|(record_id, ops)| ops.first().map(|op| (*model_id, record_id, op)))
		})
	}

	#[must_use]
	pub fn last(&self) -> Option<(ModelId, &RecordId, &CompressedCRDTOperation)> {
		self.0.last().and_then(|(model_id, data)| {
			data.last()
				.and_then(|(record_id, ops)| ops.last().map(|op| (*model_id, record_id, op)))
		})
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.0
			.iter()
			.map(|(_, data)| data.iter().map(|(_, ops)| ops.len()).sum::<usize>())
			.sum()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	#[must_use]
	pub fn into_ops(self, device_pub_id: DevicePubId) -> Vec<CRDTOperation> {
		let mut ops = Vec::with_capacity(self.len());

		for (model_id, model_messages) in self.0 {
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
	use uuid::Uuid;

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

		assert_eq!(compressed[0].1 .0[0].0, 0);
		assert_eq!(compressed[0].1 .0[1].0, 1);

		assert_eq!(compressed[0].1 .0[0].1[0].1.len(), 5);
		assert_eq!(compressed[0].1 .0[1].1[0].1.len(), 2);
	}

	#[test]
	fn into_ops() {
		let compressed = CompressedCRDTOperationsPerModelPerDevice(vec![(
			Uuid::new_v4(),
			CompressedCRDTOperationsPerModel(vec![
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
			]),
		)]);

		let uncompressed = compressed.into_ops();

		assert_eq!(uncompressed.len(), 7);
		assert_eq!(uncompressed[2].model_id, 0);
		assert_eq!(uncompressed[4].model_id, 0);
		assert_eq!(uncompressed[6].model_id, 1);
	}
}
