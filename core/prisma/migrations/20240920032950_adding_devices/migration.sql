/*
  Warnings:

  - You are about to drop the `node` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `instance_id` on the `cloud_crdt_operation` table. All the data in the column will be lost.
  - You are about to drop the column `instance_id` on the `crdt_operation` table. All the data in the column will be lost.
  - You are about to drop the column `instance_pub_id` on the `storage_statistics` table. All the data in the column will be lost.
  - Added the required column `device_pub_id` to the `cloud_crdt_operation` table without a default value. This is not possible if the table is not empty.
  - Added the required column `device_pub_id` to the `crdt_operation` table without a default value. This is not possible if the table is not empty.

*/
-- DropIndex
DROP INDEX "node_pub_id_key";

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "node";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "device" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "os" INTEGER,
    "hardware_model" INTEGER,
    "timestamp" BIGINT,
    "date_created" DATETIME,
    "date_deleted" DATETIME
);

-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_cloud_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" INTEGER NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "device_pub_id" BLOB NOT NULL,
    CONSTRAINT "cloud_crdt_operation_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_cloud_crdt_operation" ("data", "id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "kind", "model", "record_id", "timestamp" FROM "cloud_crdt_operation";
DROP TABLE "cloud_crdt_operation";
ALTER TABLE "new_cloud_crdt_operation" RENAME TO "cloud_crdt_operation";
CREATE INDEX "cloud_crdt_operation_timestamp_idx" ON "cloud_crdt_operation"("timestamp");
CREATE TABLE "new_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" INTEGER NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "device_pub_id" BLOB NOT NULL,
    CONSTRAINT "crdt_operation_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_crdt_operation" ("data", "id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "kind", "model", "record_id", "timestamp" FROM "crdt_operation";
DROP TABLE "crdt_operation";
ALTER TABLE "new_crdt_operation" RENAME TO "crdt_operation";
CREATE INDEX "crdt_operation_timestamp_idx" ON "crdt_operation"("timestamp");
CREATE TABLE "new_exif_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "resolution" BLOB,
    "media_date" BLOB,
    "media_location" BLOB,
    "camera_data" BLOB,
    "artist" TEXT,
    "description" TEXT,
    "copyright" TEXT,
    "exif_version" TEXT,
    "epoch_time" BIGINT,
    "object_id" INTEGER NOT NULL,
    "device_pub_id" BLOB,
    CONSTRAINT "exif_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "exif_data_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_exif_data" ("artist", "camera_data", "copyright", "description", "epoch_time", "exif_version", "id", "media_date", "media_location", "object_id", "resolution") SELECT "artist", "camera_data", "copyright", "description", "epoch_time", "exif_version", "id", "media_date", "media_location", "object_id", "resolution" FROM "exif_data";
DROP TABLE "exif_data";
ALTER TABLE "new_exif_data" RENAME TO "exif_data";
CREATE UNIQUE INDEX "exif_data_object_id_key" ON "exif_data"("object_id");
CREATE TABLE "new_file_path" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "is_dir" BOOLEAN,
    "cas_id" TEXT,
    "integrity_checksum" TEXT,
    "location_id" INTEGER,
    "materialized_path" TEXT,
    "name" TEXT,
    "extension" TEXT,
    "hidden" BOOLEAN,
    "size_in_bytes" TEXT,
    "size_in_bytes_bytes" BLOB,
    "inode" BLOB,
    "object_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME,
    "date_indexed" DATETIME,
    "device_pub_id" BLOB,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "file_path_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_file_path" ("cas_id", "date_created", "date_indexed", "date_modified", "extension", "hidden", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes", "size_in_bytes_bytes") SELECT "cas_id", "date_created", "date_indexed", "date_modified", "extension", "hidden", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes", "size_in_bytes_bytes" FROM "file_path";
DROP TABLE "file_path";
ALTER TABLE "new_file_path" RENAME TO "file_path";
CREATE UNIQUE INDEX "file_path_pub_id_key" ON "file_path"("pub_id");
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");
CREATE INDEX "file_path_location_id_materialized_path_idx" ON "file_path"("location_id", "materialized_path");
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");
CREATE UNIQUE INDEX "file_path_location_id_inode_key" ON "file_path"("location_id", "inode");
CREATE TABLE "new_label_on_object" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "object_id" INTEGER NOT NULL,
    "label_id" INTEGER NOT NULL,
    "device_pub_id" BLOB,

    PRIMARY KEY ("label_id", "object_id"),
    CONSTRAINT "label_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "label_on_object_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "label" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "label_on_object_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_label_on_object" ("date_created", "label_id", "object_id") SELECT "date_created", "label_id", "object_id" FROM "label_on_object";
DROP TABLE "label_on_object";
ALTER TABLE "new_label_on_object" RENAME TO "label_on_object";
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "size_in_bytes" BLOB,
    "is_archived" BOOLEAN,
    "generate_preview_media" BOOLEAN,
    "sync_preview_media" BOOLEAN,
    "hidden" BOOLEAN,
    "date_created" DATETIME,
    "scan_state" INTEGER NOT NULL DEFAULT 0,
    "device_pub_id" BLOB,
    "instance_id" INTEGER,
    CONSTRAINT "location_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "location_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "scan_state", "size_in_bytes", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "scan_state", "size_in_bytes", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
CREATE TABLE "new_object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "kind" INTEGER,
    "key_id" INTEGER,
    "hidden" BOOLEAN,
    "favorite" BOOLEAN,
    "important" BOOLEAN,
    "note" TEXT,
    "date_created" DATETIME,
    "date_accessed" DATETIME,
    "device_pub_id" BLOB,
    CONSTRAINT "object_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_object" ("date_accessed", "date_created", "favorite", "hidden", "id", "important", "key_id", "kind", "note", "pub_id") SELECT "date_accessed", "date_created", "favorite", "hidden", "id", "important", "key_id", "kind", "note", "pub_id" FROM "object";
DROP TABLE "object";
ALTER TABLE "new_object" RENAME TO "object";
CREATE UNIQUE INDEX "object_pub_id_key" ON "object"("pub_id");
CREATE TABLE "new_storage_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "total_capacity" BIGINT NOT NULL DEFAULT 0,
    "available_capacity" BIGINT NOT NULL DEFAULT 0,
    "device_pub_id" BLOB,
    CONSTRAINT "storage_statistics_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_storage_statistics" ("available_capacity", "id", "pub_id", "total_capacity") SELECT "available_capacity", "id", "pub_id", "total_capacity" FROM "storage_statistics";
DROP TABLE "storage_statistics";
ALTER TABLE "new_storage_statistics" RENAME TO "storage_statistics";
CREATE UNIQUE INDEX "storage_statistics_pub_id_key" ON "storage_statistics"("pub_id");
CREATE UNIQUE INDEX "storage_statistics_device_pub_id_key" ON "storage_statistics"("device_pub_id");
CREATE TABLE "new_tag_on_object" (
    "object_id" INTEGER NOT NULL,
    "tag_id" INTEGER NOT NULL,
    "date_created" DATETIME,
    "device_pub_id" BLOB,

    PRIMARY KEY ("tag_id", "object_id"),
    CONSTRAINT "tag_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "tag_on_object_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tag" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "tag_on_object_device_pub_id_fkey" FOREIGN KEY ("device_pub_id") REFERENCES "device" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_tag_on_object" ("date_created", "object_id", "tag_id") SELECT "date_created", "object_id", "tag_id" FROM "tag_on_object";
DROP TABLE "tag_on_object";
ALTER TABLE "new_tag_on_object" RENAME TO "tag_on_object";
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;

-- CreateIndex
CREATE UNIQUE INDEX "device_pub_id_key" ON "device"("pub_id");
