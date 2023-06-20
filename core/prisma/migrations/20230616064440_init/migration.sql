-- CreateTable
CREATE TABLE "owned_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "data" BLOB NOT NULL,
    "model" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "owned_operation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "shared_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "shared_operation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
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
    "node_id" INTEGER,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_archived" BOOLEAN,
    "generate_preview_media" BOOLEAN,
    "sync_preview_media" BOOLEAN,
    "hidden" BOOLEAN,
    "date_created" DATETIME,
    CONSTRAINT "location_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_path" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "is_dir" BOOLEAN,
    "cas_id" TEXT,
    "integrity_checksum" TEXT,
    "location_id" INTEGER,
    "materialized_path" TEXT,
    "name" TEXT,
    "extension" TEXT,
    "size_in_bytes" TEXT,
    "inode" BLOB,
    "device" BLOB,
    "object_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME,
    "date_indexed" DATETIME,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "kind" INTEGER,
    "key_id" INTEGER,
    "hidden" BOOLEAN,
    "favorite" BOOLEAN,
    "important" BOOLEAN,
    "note" TEXT,
    "date_created" DATETIME,
    "date_accessed" DATETIME
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
    "redundancy_goal" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME
);

-- CreateTable
CREATE TABLE "tag_on_object" (
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
    "date_created" DATETIME,
    "date_modified" DATETIME
);

-- CreateTable
CREATE TABLE "object_in_space" (
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
    "action" TEXT,
    "status" INTEGER NOT NULL DEFAULT 0,
    "errors_text" TEXT,
    "data" BLOB,
    "metadata" BLOB,
    "parent_id" BLOB,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_estimated_completion" DATETIME,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_started" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "date_completed" DATETIME,
    CONSTRAINT "job_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "indexer_rule" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB,
    "name" TEXT NOT NULL,
    "default" BOOLEAN NOT NULL DEFAULT false,
    "rules_per_kind" BLOB NOT NULL,
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
CREATE UNIQUE INDEX "file_path_pub_id_key" ON "file_path"("pub_id");

-- CreateIndex
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");

-- CreateIndex
CREATE INDEX "file_path_location_id_materialized_path_idx" ON "file_path"("location_id", "materialized_path");

-- CreateIndex
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");

-- CreateIndex
CREATE UNIQUE INDEX "file_path_location_id_inode_device_key" ON "file_path"("location_id", "inode", "device");

-- CreateIndex
CREATE UNIQUE INDEX "object_pub_id_key" ON "object"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "tag_pub_id_key" ON "tag"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "label_pub_id_key" ON "label"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "space_pub_id_key" ON "space"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "indexer_rule_pub_id_key" ON "indexer_rule"("pub_id");
