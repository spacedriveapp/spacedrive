#![recursion_limit = "256"]
#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use sd_prisma::prisma::{file_path, job, label, location, object};
use sd_utils::{from_bytes_to_uuid, uuid_to_bytes};

use std::{borrow::Cow, fmt};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// File Path selectables!
file_path::select!(file_path_id { id });
file_path::select!(file_path_pub_id { pub_id });
file_path::select!(file_path_pub_and_cas_ids { id pub_id cas_id });
file_path::select!(file_path_just_pub_id_materialized_path {
	pub_id
	materialized_path
});
file_path::select!(file_path_for_file_identifier {
	id
	pub_id
	materialized_path
	date_created
	is_dir
	name
	extension
	object_id
});
file_path::select!(file_path_for_object_validator {
	pub_id
	materialized_path
	is_dir
	name
	extension
	integrity_checksum
});
file_path::select!(file_path_for_media_processor {
	id
	materialized_path
	is_dir
	name
	extension
	cas_id
	object: select {
		id
		pub_id
	}
});
file_path::select!(file_path_watcher_remove {
	id
	pub_id
	location_id
	materialized_path
	is_dir
	name
	extension
	object: select {
		id
		pub_id
	}

});
file_path::select!(file_path_to_isolate {
	location_id
	materialized_path
	is_dir
	name
	extension
});
file_path::select!(file_path_to_isolate_with_pub_id {
	pub_id
	location_id
	materialized_path
	is_dir
	name
	extension
});
file_path::select!(file_path_to_isolate_with_id {
	id
	location_id
	materialized_path
	is_dir
	name
	extension
});
file_path::select!(file_path_walker {
	pub_id
	location_id
	object_id
	materialized_path
	is_dir
	name
	extension
	date_modified
	inode
	size_in_bytes_bytes
	hidden
});
file_path::select!(file_path_to_handle_custom_uri {
	pub_id
	materialized_path
	is_dir
	name
	extension
	location: select {
		id
		path
		instance: select {
			identity
			remote_identity
			node_remote_identity
		}
	}
});
file_path::select!(file_path_to_handle_p2p_serve_file {
	materialized_path
	name
	extension
	is_dir // For isolated file path
	location: select {
		id
		path
	}
});
file_path::select!(file_path_to_full_path {
	id
	materialized_path
	is_dir
	name
	extension
	location: select {
		id
		path
	}
});
file_path::select!(file_path_to_create_object {
	id
	pub_id
	date_created
});

// File Path includes!
file_path::include!(file_path_with_object { object });
file_path::include!(file_path_for_frontend {
	object: include {
		tags: include { tag }
		exif_data: select {
			resolution
			media_date
			media_location
			camera_data
			artist
			description
			copyright
			exif_version
		}
	}
});

// Object selectables!
object::select!(object_ids { id pub_id });
object::select!(object_for_file_identifier {
	pub_id
	file_paths: select { pub_id cas_id extension is_dir materialized_path name }
});

// Object includes!
object::include!(object_with_file_paths {
	file_paths: include {
		object: include {
			exif_data: select {
				resolution
				media_date
				media_location
				camera_data
				artist
				description
				copyright
				exif_version
			}
			ffmpeg_data: include {
				chapters
				programs: include {
					streams: include {
						codec: include {
							audio_props
							video_props
						}
					}
				}
			}
		}
	}
});
object::include!(object_with_media_data {
	exif_data
	ffmpeg_data: include {
		chapters
		programs: include {
			streams: include {
				codec: include {
					audio_props
					video_props
				}
			}
		}
	}
});

// Job selectables!
job::select!(job_without_data {
	id
	name
	action
	status
	parent_id
	errors_text
	metadata
	date_created
	date_started
	date_completed
	task_count
	info
	completed_task_count
	date_estimated_completion
});

// Location selectables!
location::select!(location_ids_and_path {
	id
	pub_id
	device: select { pub_id }
	path
});

// Location includes!
location::include!(location_with_indexer_rules {
	indexer_rules: select { indexer_rule }
});

impl From<location_with_indexer_rules::Data> for location::Data {
	fn from(data: location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id,
			path: data.path,
			device_id: data.device_id,
			instance_id: data.instance_id,
			name: data.name,
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			is_archived: data.is_archived,
			size_in_bytes: data.size_in_bytes,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			scan_state: data.scan_state,
			file_paths: None,
			indexer_rules: None,
			device: None,
			instance: None,
		}
	}
}

impl From<&location_with_indexer_rules::Data> for location::Data {
	fn from(data: &location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id.clone(),
			path: data.path.clone(),
			device_id: data.device_id,
			instance_id: data.instance_id,
			name: data.name.clone(),
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			size_in_bytes: data.size_in_bytes.clone(),
			is_archived: data.is_archived,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			scan_state: data.scan_state,
			file_paths: None,
			indexer_rules: None,
			device: None,
			instance: None,
		}
	}
}

// Label includes!
label::include!((take: i64) => label_with_objects {
	label_objects(vec![]).take(take): select {
		object: select {
			id
			file_paths(vec![]).take(1)
		}
	}
});

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, specta::Type)]
#[serde(transparent)]
pub struct CasId<'cas_id>(Cow<'cas_id, str>);

impl Clone for CasId<'_> {
	fn clone(&self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}
}

impl CasId<'_> {
	#[must_use]
	pub fn as_str(&self) -> &str {
		self.0.as_ref()
	}

	#[must_use]
	pub fn to_owned(&self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}

	#[must_use]
	pub fn into_owned(self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}
}

impl From<&CasId<'_>> for file_path::cas_id::Type {
	fn from(CasId(cas_id): &CasId<'_>) -> Self {
		Some(cas_id.clone().into_owned())
	}
}

impl<'cas_id> From<&'cas_id str> for CasId<'cas_id> {
	fn from(cas_id: &'cas_id str) -> Self {
		Self(Cow::Borrowed(cas_id))
	}
}

impl<'cas_id> From<&'cas_id String> for CasId<'cas_id> {
	fn from(cas_id: &'cas_id String) -> Self {
		Self(Cow::Borrowed(cas_id))
	}
}

impl From<String> for CasId<'static> {
	fn from(cas_id: String) -> Self {
		Self(cas_id.into())
	}
}

impl From<CasId<'_>> for String {
	fn from(CasId(cas_id): CasId<'_>) -> Self {
		cas_id.into_owned()
	}
}

impl From<&CasId<'_>> for String {
	fn from(CasId(cas_id): &CasId<'_>) -> Self {
		cas_id.clone().into_owned()
	}
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, specta::Type)]
#[serde(transparent)]
#[repr(transparent)]
#[specta(rename = "CoreDevicePubId")]
pub struct DevicePubId(PubId);

impl From<DevicePubId> for sd_cloud_schema::devices::PubId {
	fn from(DevicePubId(pub_id): DevicePubId) -> Self {
		Self(pub_id.into())
	}
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, specta::Type)]
#[serde(transparent)]
#[repr(transparent)]
#[specta(rename = "CoreFilePathPubId")]
pub struct FilePathPubId(PubId);

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, specta::Type)]
#[serde(transparent)]
#[repr(transparent)]
#[specta(rename = "CoreObjectPubId")]
pub struct ObjectPubId(PubId);

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, specta::Type)]
#[specta(rename = "CorePubId")]
enum PubId {
	Uuid(Uuid),
	Vec(Vec<u8>),
}

impl PubId {
	fn new() -> Self {
		Self::Uuid(Uuid::now_v7())
	}

	fn to_db(&self) -> Vec<u8> {
		match self {
			Self::Uuid(uuid) => uuid_to_bytes(uuid),
			Self::Vec(bytes) => bytes.clone(),
		}
	}
}

impl Default for PubId {
	fn default() -> Self {
		Self::new()
	}
}

impl From<Uuid> for PubId {
	fn from(uuid: Uuid) -> Self {
		Self::Uuid(uuid)
	}
}

impl From<Vec<u8>> for PubId {
	fn from(bytes: Vec<u8>) -> Self {
		Self::Vec(bytes)
	}
}

impl From<&Vec<u8>> for PubId {
	fn from(bytes: &Vec<u8>) -> Self {
		Self::Vec(bytes.clone())
	}
}

impl From<&[u8]> for PubId {
	fn from(bytes: &[u8]) -> Self {
		Self::Vec(bytes.to_vec())
	}
}

impl From<PubId> for Vec<u8> {
	fn from(pub_id: PubId) -> Self {
		match pub_id {
			PubId::Uuid(uuid) => uuid_to_bytes(&uuid),
			PubId::Vec(bytes) => bytes,
		}
	}
}

impl From<PubId> for Uuid {
	fn from(pub_id: PubId) -> Self {
		match pub_id {
			PubId::Uuid(uuid) => uuid,
			PubId::Vec(bytes) => from_bytes_to_uuid(&bytes),
		}
	}
}

impl From<&PubId> for Uuid {
	fn from(pub_id: &PubId) -> Self {
		match pub_id {
			PubId::Uuid(uuid) => *uuid,
			PubId::Vec(bytes) => from_bytes_to_uuid(bytes),
		}
	}
}

impl fmt::Display for PubId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Uuid(uuid) => write!(f, "{uuid}"),
			Self::Vec(bytes) => write!(f, "{}", from_bytes_to_uuid(bytes)),
		}
	}
}

macro_rules! delegate_pub_id {
	($($type_name:ty),+ $(,)?) => {
		$(
			impl From<::uuid::Uuid> for $type_name {
				fn from(uuid: ::uuid::Uuid) -> Self {
					Self(uuid.into())
				}
			}

			impl From<Vec<u8>> for $type_name {
				fn from(bytes: Vec<u8>) -> Self {
					Self(bytes.into())
				}
			}

			impl From<&Vec<u8>> for $type_name {
				fn from(bytes: &Vec<u8>) -> Self {
					Self(bytes.into())
				}
			}

			impl From<&[u8]> for $type_name {
				fn from(bytes: &[u8]) -> Self {
					Self(bytes.into())
				}
			}

			impl From<$type_name> for Vec<u8> {
				fn from(pub_id: $type_name) -> Self {
					pub_id.0.into()
				}
			}

			impl From<$type_name> for ::uuid::Uuid {
				fn from(pub_id: $type_name) -> Self {
					pub_id.0.into()
				}
			}

			impl From<&$type_name> for ::uuid::Uuid {
				fn from(pub_id: &$type_name) -> Self {
					(&pub_id.0).into()
				}
			}

			impl ::std::fmt::Display for $type_name {
				fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
					write!(f, "{}", self.0)
				}
			}

			impl $type_name {
				#[must_use]
				pub fn new() -> Self {
					Self(PubId::new())
				}

				#[must_use]
				pub fn to_db(&self) -> Vec<u8> {
					self.0.to_db()
				}
			}

			impl Default for $type_name {
				fn default() -> Self {
					Self::new()
				}
			}
		)+
	};
}

delegate_pub_id!(FilePathPubId, ObjectPubId, DevicePubId);
