#[derive(Serialize, Deserialize)]
pub struct CompressedCRDTOperations(
	Vec<(
		Uuid,
		Vec<(String, Vec<(Value, Vec<CompressedCRDTOperation>)>)>,
	)>,
);

impl CompressedCRDTOperations {
	pub fn new(ops: Vec<CRDTOperation>) -> Self {
		let mut compressed = vec![];

		let mut ops_iter = ops.into_iter();

		let Some(first) = ops_iter.next() else {
			return Self(vec![]);
		};

		let mut instance_id = first.instance;
		let mut instance = vec![];

		let mut model_str = first.model.clone();
		let mut model = vec![];

		let mut record_id = first.record_id.clone();
		let mut record = vec![first.into()];

		for op in ops_iter {
			if instance_id != op.instance {
				model.push((
					std::mem::replace(&mut record_id, op.record_id),
					std::mem::take(&mut record),
				));
				instance.push((
					std::mem::replace(&mut model_str, op.model),
					std::mem::take(&mut model),
				));
				compressed.push((
					std::mem::replace(&mut instance_id, op.instance),
					std::mem::take(&mut instance),
				));
			} else if model_str != op.model {
				model.push((
					std::mem::replace(&mut record_id, op.record_id),
					std::mem::take(&mut record),
				));
				instance.push((
					std::mem::replace(&mut model_str, op.model),
					std::mem::take(&mut model),
				));
			} else if record_id != op.record_id {
				model.push((
					std::mem::replace(&mut record_id, op.record_id),
					std::mem::take(&mut record),
				));
			}

			record.push(CompressedCRDTOperation::from(op))
		}

		model.push((record_id, record));
		instance.push((model_str, model));
		compressed.push((instance_id, instance));

		Self(compressed)
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct CompressedCRDTOperation {
	pub timestamp: NTP64,
	pub id: Uuid,
	pub data: CRDTOperationData,
}

impl From<CRDTOperation> for CompressedCRDTOperation {
	fn from(value: CRDTOperation) -> Self {
		Self {
			timestamp: value.timestamp,
			id: value.id,
			data: value.data,
		}
	}
}
