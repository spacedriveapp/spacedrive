CREATE TABLE IF NOT EXISTS files (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	uri STRING NOT NULL,
	meta_checksum STRING NOT NULL,
	buffer_checksum STRING,
	name STRING,
	extension STRING,
	size_in_bytes STRING NOT NULL,
	encryption INTEGER DEFAULT 0,
	ipfs_id STRING,
	date_created TEXT NOT NULL,
	date_modified TEXT NOT NULL,
	date_indexed TEXT NOT NULL,
    storage_device_id STRING,
    capture_device_id STRING,
    parent_file_id STRING
);