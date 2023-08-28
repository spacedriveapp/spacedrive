/*
  Warnings:

  - You are about to drop the column `capture_device_make` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_model` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_software` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `codecs` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `duration_seconds` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `fps` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `latitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `longitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_height` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_width` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `streams` on the `media_data` table. All the data in the column will be lost.
  - Added the required column `object_id` to the `media_data` table without a default value. This is not possible if the table is not empty.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_label_on_object" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "label_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("label_id", "object_id"),
    CONSTRAINT "label_on_object_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "label" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "label_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_label_on_object" ("date_created", "label_id", "object_id") SELECT "date_created", "label_id", "object_id" FROM "label_on_object";
DROP TABLE "label_on_object";
ALTER TABLE "new_label_on_object" RENAME TO "label_on_object";
CREATE TABLE "new_tag_on_object" (
    "tag_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("tag_id", "object_id"),
    CONSTRAINT "tag_on_object_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tag" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "tag_on_object_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_tag_on_object" ("object_id", "tag_id") SELECT "object_id", "tag_id" FROM "tag_on_object";
DROP TABLE "tag_on_object";
ALTER TABLE "new_tag_on_object" RENAME TO "tag_on_object";
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
    "size_in_bytes" TEXT,
    "size_in_bytes_bytes" BLOB,
    "inode" BLOB,
    "device" BLOB,
    "object_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME,
    "date_indexed" DATETIME,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_path" ("cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes", "size_in_bytes_bytes") SELECT "cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes", "size_in_bytes_bytes" FROM "file_path";
DROP TABLE "file_path";
ALTER TABLE "new_file_path" RENAME TO "file_path";
CREATE UNIQUE INDEX "file_path_pub_id_key" ON "file_path"("pub_id");
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");
CREATE INDEX "file_path_location_id_materialized_path_idx" ON "file_path"("location_id", "materialized_path");
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");
CREATE UNIQUE INDEX "file_path_location_id_inode_device_key" ON "file_path"("location_id", "inode", "device");
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_archived" BOOLEAN,
    "generate_preview_media" BOOLEAN,
    "sync_preview_media" BOOLEAN,
    "hidden" BOOLEAN,
    "date_created" DATETIME,
    "instance_id" INTEGER,
    CONSTRAINT "location_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
CREATE TABLE "new_job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT,
    "action" TEXT,
    "status" INTEGER,
    "errors_text" TEXT,
    "data" BLOB,
    "metadata" BLOB,
    "parent_id" BLOB,
    "task_count" INTEGER,
    "completed_task_count" INTEGER,
    "date_estimated_completion" DATETIME,
    "date_created" DATETIME,
    "date_started" DATETIME,
    "date_completed" DATETIME,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
CREATE TABLE "new_indexer_rule_in_location" (
    "location_id" INTEGER NOT NULL,
    "indexer_rule_id" INTEGER NOT NULL,

    PRIMARY KEY ("location_id", "indexer_rule_id"),
    CONSTRAINT "indexer_rule_in_location_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "indexer_rule_in_location_indexer_rule_id_fkey" FOREIGN KEY ("indexer_rule_id") REFERENCES "indexer_rule" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_indexer_rule_in_location" ("indexer_rule_id", "location_id") SELECT "indexer_rule_id", "location_id" FROM "indexer_rule_in_location";
DROP TABLE "indexer_rule_in_location";
ALTER TABLE "new_indexer_rule_in_location" RENAME TO "indexer_rule_in_location";
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "dimensions" BLOB,
    "media_date" BLOB,
    "media_location" BLOB,
    "camera_data" BLOB,
    "artist" TEXT,
    "description" TEXT,
    "copyright" TEXT,
    "exif_version" TEXT,
    "object_id" INTEGER NOT NULL,
    CONSTRAINT "media_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("id") SELECT "id" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
CREATE UNIQUE INDEX "media_data_object_id_key" ON "media_data"("object_id");
CREATE TABLE "new_object_in_space" (
    "space_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("space_id", "object_id"),
    CONSTRAINT "object_in_space_space_id_fkey" FOREIGN KEY ("space_id") REFERENCES "space" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "object_in_space_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_object_in_space" ("object_id", "space_id") SELECT "object_id", "space_id" FROM "object_in_space";
DROP TABLE "object_in_space";
ALTER TABLE "new_object_in_space" RENAME TO "object_in_space";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
