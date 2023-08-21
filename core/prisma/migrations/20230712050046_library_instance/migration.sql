/*
  Warnings:

  - You are about to drop the column `node_id` on the `job` table. All the data in the column will be lost.
  - You are about to drop the column `node_id` on the `shared_operation` table. All the data in the column will be lost.
  - You are about to drop the column `node_id` on the `location` table. All the data in the column will be lost.
  - You are about to drop the column `node_id` on the `volume` table. All the data in the column will be lost.
  - Added the required column `instance_id` to the `shared_operation` table without a default value. This is not possible if the table is not empty.

*/
-- CreateTable
CREATE TABLE "instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "identity" BLOB NOT NULL,
    "node_id" BLOB NOT NULL,
    "node_name" TEXT NOT NULL,
    "node_platform" INTEGER NOT NULL,
    "last_seen" DATETIME NOT NULL,
    "date_created" DATETIME NOT NULL
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
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
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
CREATE TABLE "new_shared_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "shared_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
-- INSERT INTO "new_shared_operation" ("data", "id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "kind", "model", "record_id", "timestamp" FROM "shared_operation";
DROP TABLE "shared_operation";
ALTER TABLE "new_shared_operation" RENAME TO "shared_operation";
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
    "instance_id" INTEGER
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "path", "pub_id", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "path", "pub_id", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
CREATE TABLE "new_volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "is_system" BOOLEAN NOT NULL DEFAULT false,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_volume" ("date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "volume";
DROP TABLE "volume";
ALTER TABLE "new_volume" RENAME TO "volume";
CREATE UNIQUE INDEX "volume_mount_point_name_key" ON "volume"("mount_point", "name");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "instance_pub_id_key" ON "instance"("pub_id");
