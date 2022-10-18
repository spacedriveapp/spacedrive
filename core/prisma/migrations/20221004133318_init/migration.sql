-- CreateTable
CREATE TABLE "sync_event" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "node_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" INTEGER NOT NULL,
    "column" TEXT,
    "value" TEXT NOT NULL,
    CONSTRAINT "sync_event_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "total_object_count" INTEGER NOT NULL DEFAULT 0,
    "library_db_size" TEXT NOT NULL DEFAULT '0',
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "total_bytes_free" TEXT NOT NULL DEFAULT '0',
    "preview_media_bytes" TEXT NOT NULL DEFAULT '0'
);

-- CreateTable
CREATE TABLE "node" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL DEFAULT 0,
    "version" TEXT,
    "last_seen" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "node_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "is_system" BOOLEAN NOT NULL DEFAULT false,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "is_archived" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "location_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "name" TEXT,
    "extension" TEXT COLLATE NOCASE,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
    "key_id" INTEGER,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbnail" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "note" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "object_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_path" (
    "id" INTEGER NOT NULL,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT COLLATE NOCASE,
    "object_id" INTEGER,
    "parent_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY ("location_id", "id"),
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_conflict" (
    "original_object_id" INTEGER NOT NULL,
    "detactched_object_id" INTEGER NOT NULL
);

-- CreateTable
CREATE TABLE "key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "checksum" TEXT NOT NULL,
    "name" TEXT,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "algorithm" INTEGER DEFAULT 0
);

-- CreateTable
CREATE TABLE "media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pixel_width" INTEGER,
    "pixel_height" INTEGER,
    "longitude" REAL,
    "latitude" REAL,
    "fps" INTEGER,
    "capture_device_make" TEXT,
    "capture_device_model" TEXT,
    "capture_device_software" TEXT,
    "duration_seconds" INTEGER,
    "codecs" TEXT,
    "streams" INTEGER,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "tag" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "total_objects" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "tag_on_object" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "tag_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("tag_id", "object_id"),
    CONSTRAINT "tag_on_object_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tag" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "tag_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "label" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "label_on_object" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "label_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("label_id", "object_id"),
    CONSTRAINT "label_on_object_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "label" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "label_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "space" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "description" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "object_in_space" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "space_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("space_id", "object_id"),
    CONSTRAINT "object_in_space_space_id_fkey" FOREIGN KEY ("space_id") REFERENCES "space" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "object_in_space_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "data" BLOB,
    "metadata" BLOB,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "seconds_elapsed" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "job_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "album" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "object_in_album" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "album_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("album_id", "object_id"),
    CONSTRAINT "object_in_album_album_id_fkey" FOREIGN KEY ("album_id") REFERENCES "album" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "object_in_album_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "comment" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "object_id" INTEGER,
    CONSTRAINT "comment_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "indexer_rule" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "parameters" BLOB NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "indexer_rule_in_location" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "location_id" INTEGER NOT NULL,
    "indexer_rule_id" INTEGER NOT NULL,

    PRIMARY KEY ("location_id", "indexer_rule_id"),
    CONSTRAINT "indexer_rule_in_location_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "indexer_rule_in_location_indexer_rule_id_fkey" FOREIGN KEY ("indexer_rule_id") REFERENCES "indexer_rule" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateIndex
CREATE UNIQUE INDEX "node_pub_id_key" ON "node"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "volume_node_id_mount_point_name_key" ON "volume"("node_id", "mount_point", "name");

-- CreateIndex
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "object_cas_id_key" ON "object"("cas_id");

-- CreateIndex
CREATE UNIQUE INDEX "object_integrity_checksum_key" ON "object"("integrity_checksum");

-- CreateIndex
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");

-- CreateIndex
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");

-- CreateIndex
CREATE UNIQUE INDEX "file_conflict_original_object_id_key" ON "file_conflict"("original_object_id");

-- CreateIndex
CREATE UNIQUE INDEX "file_conflict_detactched_object_id_key" ON "file_conflict"("detactched_object_id");

-- CreateIndex
CREATE UNIQUE INDEX "key_checksum_key" ON "key"("checksum");

-- CreateIndex
CREATE UNIQUE INDEX "tag_pub_id_key" ON "tag"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "label_pub_id_key" ON "label"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "space_pub_id_key" ON "space"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "album_pub_id_key" ON "album"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "comment_pub_id_key" ON "comment"("pub_id");
