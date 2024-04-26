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

use sd_prisma::prisma::{self, file_path, job, label, location, object};

// File Path selectables!
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
	object_id
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

// File Path includes!
file_path::include!(file_path_with_object {
	object: include {
		media_data: select {
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
object::select!(object_for_file_identifier {
	pub_id
	file_paths: select { pub_id cas_id extension is_dir materialized_path name }
});

// Object includes!
object::include!(object_with_file_paths {
	file_paths: include {
		object: include {
			media_data: select {
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
	}
});

impl sd_cache::Model for object_with_file_paths::file_paths::Data {
	fn name() -> &'static str {
		// This is okay because it's a superset of the available fields.
		prisma::file_path::NAME
	}
}

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
	completed_task_count
	date_estimated_completion
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
