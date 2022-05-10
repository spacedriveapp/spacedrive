/*
  Warnings:

  - You are about to drop the `clients` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `client_id` on the `sync_events` table. All the data in the column will be lost.
  - You are about to drop the column `client_id` on the `locations` table. All the data in the column will be lost.
  - You are about to drop the column `client_id` on the `jobs` table. All the data in the column will be lost.
  - You are about to drop the column `encryption` on the `tags` table. All the data in the column will be lost.
  - You are about to drop the column `client_id` on the `volumes` table. All the data in the column will be lost.
  - Added the required column `node_id` to the `sync_events` table without a default value. This is not possible if the table is not empty.
  - Added the required column `node_id` to the `jobs` table without a default value. This is not possible if the table is not empty.
  - Added the required column `node_id` to the `volumes` table without a default value. This is not possible if the table is not empty.

*/
-- DropIndex
DROP INDEX "clients_pub_id_key";

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "clients";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "nodes" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL DEFAULT 0,
    "version" TEXT,
    "online" BOOLEAN DEFAULT true,
    "last_seen" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "keys" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "checksum" TEXT NOT NULL,
    "name" TEXT,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "algorithm" INTEGER DEFAULT 0
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "key_id" INTEGER,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbnail" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "comment" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "files_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_files" ("cas_id", "comment", "date_created", "date_indexed", "date_modified", "encryption", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "kind", "size_in_bytes") SELECT "cas_id", "comment", "date_created", "date_indexed", "date_modified", "encryption", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "kind", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_cas_id_key" ON "files"("cas_id");
CREATE UNIQUE INDEX "files_integrity_checksum_key" ON "files"("integrity_checksum");
CREATE TABLE "new_sync_events" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "node_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "data" TEXT NOT NULL,
    CONSTRAINT "sync_events_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_sync_events" ("data", "id", "timestamp") SELECT "data", "id", "timestamp" FROM "sync_events";
DROP TABLE "sync_events";
ALTER TABLE "new_sync_events" RENAME TO "sync_events";
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "node_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_locations" ("available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "pub_id", "total_capacity") SELECT "available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "pub_id", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");
CREATE TABLE "new_jobs" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "node_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "seconds_elapsed" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "jobs_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_jobs" ("action", "completed_task_count", "date_created", "date_modified", "id", "seconds_elapsed", "status", "task_count") SELECT "action", "completed_task_count", "date_created", "date_modified", "id", "seconds_elapsed", "status", "task_count" FROM "jobs";
DROP TABLE "jobs";
ALTER TABLE "new_jobs" RENAME TO "jobs";
CREATE TABLE "new_tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT,
    "total_files" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_tags" ("date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_files") SELECT "date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_files" FROM "tags";
DROP TABLE "tags";
ALTER TABLE "new_tags" RENAME TO "tags";
CREATE UNIQUE INDEX "tags_pub_id_key" ON "tags"("pub_id");
CREATE TABLE "new_file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "key_id" INTEGER,
    "permissions" TEXT,
    "temp_cas_id" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "file_paths_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_created", "date_indexed", "date_modified", "encryption", "extension", "file_id", "id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "permissions", "temp_cas_id") SELECT "date_created", "date_indexed", "date_modified", "encryption", "extension", "file_id", "id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "permissions", "temp_cas_id" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");
CREATE TABLE "new_volumes" (
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
INSERT INTO "new_volumes" ("date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "volumes";
DROP TABLE "volumes";
ALTER TABLE "new_volumes" RENAME TO "volumes";
CREATE UNIQUE INDEX "volumes_node_id_mount_point_name_key" ON "volumes"("node_id", "mount_point", "name");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "nodes_pub_id_key" ON "nodes"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "keys_checksum_key" ON "keys"("checksum");
