-- CreateTable
CREATE TABLE "_migrations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "checksum" TEXT NOT NULL,
    "steps_applied" INTEGER NOT NULL DEFAULT 0,
    "applied_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "sync_events" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "node_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" INTEGER NOT NULL,
    "column" TEXT,
    "value" TEXT NOT NULL,
    CONSTRAINT "sync_events_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "library_db_size" TEXT NOT NULL DEFAULT '0',
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "total_bytes_free" TEXT NOT NULL DEFAULT '0',
    "preview_media_bytes" TEXT NOT NULL DEFAULT '0'
);

-- CreateTable
CREATE TABLE "nodes" (
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
CREATE TABLE "volumes" (
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
CREATE TABLE "locations" (
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
    CONSTRAINT "locations_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "name" TEXT,
    "extension" TEXT,
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
    CONSTRAINT "files_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_paths" (
    "id" INTEGER NOT NULL,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY ("location_id", "id"),
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_conflicts" (
    "original_file_id" INTEGER NOT NULL,
    "detactched_file_id" INTEGER NOT NULL
);

-- CreateTable
CREATE TABLE "keys" (
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
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "total_files" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "tags_on_file" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "tag_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("tag_id", "file_id"),
    CONSTRAINT "tags_on_file_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tags" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "tags_on_file_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "labels" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "label_on_file" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "label_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("label_id", "file_id"),
    CONSTRAINT "label_on_file_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "labels" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "label_on_file_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "spaces" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "description" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "file_in_space" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "space_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("space_id", "file_id"),
    CONSTRAINT "file_in_space_space_id_fkey" FOREIGN KEY ("space_id") REFERENCES "spaces" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "file_in_space_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "jobs" (
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
    CONSTRAINT "jobs_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "albums" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "files_in_albums" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "album_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("album_id", "file_id"),
    CONSTRAINT "files_in_albums_album_id_fkey" FOREIGN KEY ("album_id") REFERENCES "albums" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "files_in_albums_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "comments" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "file_id" INTEGER,
    CONSTRAINT "comments_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "indexer_rules" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "parameters" BLOB NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "indexer_rules_in_location" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "location_id" INTEGER NOT NULL,
    "indexer_rule_id" INTEGER NOT NULL,

    PRIMARY KEY ("location_id", "indexer_rule_id"),
    CONSTRAINT "indexer_rules_in_location_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "indexer_rules_in_location_indexer_rule_id_fkey" FOREIGN KEY ("indexer_rule_id") REFERENCES "indexer_rules" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateIndex
CREATE UNIQUE INDEX "_migrations_checksum_key" ON "_migrations"("checksum");

-- CreateIndex
CREATE UNIQUE INDEX "nodes_pub_id_key" ON "nodes"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "volumes_node_id_mount_point_name_key" ON "volumes"("node_id", "mount_point", "name");

-- CreateIndex
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "files_cas_id_key" ON "files"("cas_id");

-- CreateIndex
CREATE UNIQUE INDEX "files_integrity_checksum_key" ON "files"("integrity_checksum");

-- CreateIndex
CREATE INDEX "file_paths_location_id_idx" ON "file_paths"("location_id");

-- CreateIndex
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");

-- CreateIndex
CREATE UNIQUE INDEX "file_conflicts_original_file_id_key" ON "file_conflicts"("original_file_id");

-- CreateIndex
CREATE UNIQUE INDEX "file_conflicts_detactched_file_id_key" ON "file_conflicts"("detactched_file_id");

-- CreateIndex
CREATE UNIQUE INDEX "keys_checksum_key" ON "keys"("checksum");

-- CreateIndex
CREATE UNIQUE INDEX "tags_pub_id_key" ON "tags"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "labels_pub_id_key" ON "labels"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "spaces_pub_id_key" ON "spaces"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "albums_pub_id_key" ON "albums"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "comments_pub_id_key" ON "comments"("pub_id");
