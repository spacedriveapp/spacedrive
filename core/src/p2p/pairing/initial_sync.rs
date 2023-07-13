use sd_prisma::prisma::*;

// TODO: Turn this entire file into a Prisma generator cause it could be way more maintainable

// Pairing will fail if the two clients aren't on versions with identical DB models so it's safe to send them and ignore migrations.

const ITEMS_PER_BATCH: i64 = 1000;

macro_rules! impl_for_models {
	($($variant:ident($model:ident)),* $(,)+) => {
		/// Represents any DB model to be ingested into the database as part of the initial sync
		#[derive(Debug, serde::Serialize, serde::Deserialize)]
		pub enum ModelData {
			$(
				$variant(Vec<$model::Data>),
			)*
		}

		impl ModelData {
			/// Length of data
			pub fn len(&self) -> usize {
				match self {
					$(
						Self::$variant(data) => data.len(),
					)*
				}
			}

			/// Get count of all of the rows in the database
			pub async fn total_count(db: &PrismaClient) -> Result<i64, prisma_client_rust::QueryError> {
				let mut total_count = 0;

				let ($( $model ),*) = tokio::join!(
					$(
						db.$model().count(vec![]).exec(),
					)*
				);

				$(total_count += $model?;)*
				Ok(total_count)
			}

			/// Insert the data into the database
			pub async fn insert(self, db: &PrismaClient) -> Result<(), prisma_client_rust::QueryError> {
				match self {
					$(
						Self::$variant(data) => {
							db.$model().create_many(data.into_iter().map(|v| FromData(v).into()).collect()).exec().await?;
						}
					)*
				}

				Ok(())
			}
		}

		/// This exists to determine the next model to sync.
		/// It emulates `.window()` functionality but for a `macro_rules`
		// TODO: When replacing with a generator this can be removed and done at compile time
		#[derive(Debug)]
		enum ModelSyncCursorIterator {
			Done = 0,
			$(
				$variant,
			)*
		}

		impl<'a> From<&'a ModelSyncCursor> for ModelSyncCursorIterator {
			fn from(cursor: &'a ModelSyncCursor) -> Self {
				match cursor {
					$(
						ModelSyncCursor::$variant(_) => Self::$variant,
					)*
					ModelSyncCursor::Done => Self::Done,
				}
			}
		}

		impl ModelSyncCursorIterator {
			pub fn next(self) -> ModelSyncCursor {
				let i = self as i32;
				match i + 1 {
					$(
						v if v == Self::$variant as i32 => ModelSyncCursor::$variant(0),
					)*
					_ => ModelSyncCursor::Done,
				}
			}
		}

		/// Represent where we ar eup to with the sync
		#[derive(Debug, serde::Serialize, serde::Deserialize)]
		pub enum ModelSyncCursor {
			$(
				$variant(i64),
			)*
			Done,
		}

		impl ModelSyncCursor {
			pub fn new() -> Self {
				new_impl!($( $variant ),*)
			}

			pub async fn next(&mut self, db: &PrismaClient) -> Option<Result<ModelData, prisma_client_rust::QueryError>> {
				match self {
					$(
						Self::$variant(cursor) => {
							match db.$model()
								.find_many(vec![])
								.skip(*cursor)
								.take(ITEMS_PER_BATCH + 1)
								.exec()
								.await {
								Ok(data) => {
									if data.len() <= ITEMS_PER_BATCH as usize {
										*self = ModelSyncCursorIterator::from(&*self).next();
									} else {
										*self = Self::$variant(*cursor + ITEMS_PER_BATCH);
									}

									Some(Ok(ModelData::$variant(data)))
								},
								Err(e) => return Some(Err(e)),
							}
						},
					)*
					Self::Done => None
				}
			}
		}
	};
}

macro_rules! new_impl {
	($x:ident, $($y:ident),+) => {
		Self::$x(0)
	};
}

impl PartialEq for ModelData {
	// Crude EQ impl based only on ID's not struct content.
	// It's super annoying PCR does have this impl but it kinda makes sense with relation fetching.
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(ModelData::SharedOperation(a), ModelData::SharedOperation(b)) => a
				.iter()
				.map(|x| x.id.clone())
				.eq(b.iter().map(|x| x.id.clone())),
			(ModelData::Volume(a), ModelData::Volume(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::Location(a), ModelData::Location(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::FilePath(a), ModelData::FilePath(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::Object(a), ModelData::Object(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::Tag(a), ModelData::Tag(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::TagOnObject(a), ModelData::TagOnObject(b)) => a
				.iter()
				.map(|x| (x.tag_id, x.object_id))
				.eq(b.iter().map(|x| (x.tag_id, x.object_id))),
			(ModelData::IndexerRule(a), ModelData::IndexerRule(b)) => {
				a.iter().map(|x| x.id).eq(b.iter().map(|x| x.id))
			}
			(ModelData::IndexerRulesInLocation(a), ModelData::IndexerRulesInLocation(b)) => a
				.iter()
				.map(|x| (x.location_id, x.indexer_rule_id))
				.eq(b.iter().map(|x| (x.location_id, x.indexer_rule_id))),
			(ModelData::Preference(a), ModelData::Preference(b)) => a
				.iter()
				.map(|x| (x.key.clone(), x.value.clone()))
				.eq(b.iter().map(|x| (x.key.clone(), x.value.clone()))),
			_ => false,
		}
	}
}

/// Meaningless wrapper to avoid Rust's orphan rule
struct FromData<T>(T);

impl From<FromData<shared_operation::Data>> for shared_operation::CreateUnchecked {
	fn from(FromData(data): FromData<shared_operation::Data>) -> Self {
		Self {
			id: data.id,
			timestamp: data.timestamp,
			model: data.model,
			record_id: data.record_id,
			kind: data.kind,
			data: data.data,
			instance_id: data.instance_id,
			_params: vec![],
		}
	}
}

impl From<FromData<volume::Data>> for volume::CreateUnchecked {
	fn from(FromData(data): FromData<volume::Data>) -> Self {
		Self {
			name: data.name,
			mount_point: data.mount_point,
			_params: vec![
				volume::id::set(data.id),
				volume::total_bytes_capacity::set(data.total_bytes_capacity),
				volume::total_bytes_available::set(data.total_bytes_available),
				volume::disk_type::set(data.disk_type),
				volume::filesystem::set(data.filesystem),
				volume::is_system::set(data.is_system),
				volume::date_modified::set(data.date_modified),
			],
		}
	}
}

impl From<FromData<location::Data>> for location::CreateUnchecked {
	fn from(FromData(data): FromData<location::Data>) -> Self {
		Self {
			pub_id: data.pub_id,
			_params: vec![
				location::id::set(data.id),
				location::name::set(data.name),
				location::path::set(data.path),
				location::total_capacity::set(data.total_capacity),
				location::available_capacity::set(data.available_capacity),
				location::is_archived::set(data.is_archived),
				location::generate_preview_media::set(data.generate_preview_media),
				location::sync_preview_media::set(data.sync_preview_media),
				location::hidden::set(data.hidden),
				location::date_created::set(data.date_created),
				location::instance_id::set(data.instance_id),
			],
		}
	}
}

impl From<FromData<file_path::Data>> for file_path::CreateUnchecked {
	fn from(FromData(data): FromData<file_path::Data>) -> Self {
		Self {
			pub_id: data.pub_id,
			_params: vec![
				file_path::id::set(data.id),
				file_path::is_dir::set(data.is_dir),
				file_path::cas_id::set(data.cas_id),
				file_path::integrity_checksum::set(data.integrity_checksum),
				file_path::location_id::set(data.location_id),
				file_path::materialized_path::set(data.materialized_path),
				file_path::name::set(data.name),
				file_path::extension::set(data.extension),
				file_path::size_in_bytes::set(data.size_in_bytes),
				file_path::size_in_bytes_bytes::set(data.size_in_bytes_bytes),
				file_path::inode::set(data.inode),
				file_path::device::set(data.device),
				file_path::object_id::set(data.object_id),
				file_path::key_id::set(data.key_id),
				file_path::date_created::set(data.date_created),
				file_path::date_modified::set(data.date_modified),
				file_path::date_indexed::set(data.date_indexed),
			],
		}
	}
}

impl From<FromData<object::Data>> for object::CreateUnchecked {
	fn from(FromData(data): FromData<object::Data>) -> Self {
		Self {
			pub_id: data.pub_id,
			_params: vec![
				object::id::set(data.id),
				object::kind::set(data.kind),
				object::key_id::set(data.key_id),
				object::hidden::set(data.hidden),
				object::favorite::set(data.favorite),
				object::important::set(data.important),
				object::note::set(data.note),
				object::date_created::set(data.date_created),
				object::date_accessed::set(data.date_accessed),
			],
		}
	}
}

impl From<FromData<tag::Data>> for tag::CreateUnchecked {
	fn from(FromData(data): FromData<tag::Data>) -> Self {
		Self {
			pub_id: data.pub_id,
			_params: vec![
				tag::id::set(data.id),
				tag::name::set(data.name),
				tag::color::set(data.color),
				tag::redundancy_goal::set(data.redundancy_goal),
				tag::date_created::set(data.date_created),
				tag::date_modified::set(data.date_modified),
			],
		}
	}
}

impl From<FromData<tag_on_object::Data>> for tag_on_object::CreateUnchecked {
	fn from(FromData(data): FromData<tag_on_object::Data>) -> Self {
		Self {
			tag_id: data.tag_id,
			object_id: data.object_id,
			_params: vec![],
		}
	}
}

impl From<FromData<indexer_rule::Data>> for indexer_rule::CreateUnchecked {
	fn from(FromData(data): FromData<indexer_rule::Data>) -> Self {
		Self {
			pub_id: data.pub_id,
			_params: vec![
				indexer_rule::id::set(data.id),
				indexer_rule::name::set(data.name),
				indexer_rule::default::set(data.default),
				indexer_rule::rules_per_kind::set(data.rules_per_kind),
				indexer_rule::date_created::set(data.date_created),
				indexer_rule::date_modified::set(data.date_modified),
			],
		}
	}
}

impl From<FromData<indexer_rules_in_location::Data>>
	for indexer_rules_in_location::CreateUnchecked
{
	fn from(FromData(data): FromData<indexer_rules_in_location::Data>) -> Self {
		Self {
			location_id: data.location_id,
			indexer_rule_id: data.indexer_rule_id,
			_params: vec![],
		}
	}
}

impl From<FromData<preference::Data>> for preference::CreateUnchecked {
	fn from(FromData(data): FromData<preference::Data>) -> Self {
		Self {
			key: data.key,
			_params: vec![preference::value::set(data.value)],
		}
	}
}

impl_for_models! {
	SharedOperation(shared_operation),
	Volume(volume),
	Location(location),
	FilePath(file_path),
	Object(object),
	Tag(tag),
	TagOnObject(tag_on_object),
	IndexerRule(indexer_rule),
	IndexerRulesInLocation(indexer_rules_in_location),
	Preference(preference),
}
