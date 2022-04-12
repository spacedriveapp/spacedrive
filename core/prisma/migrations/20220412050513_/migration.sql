/*
  Warnings:

  - You are about to drop the column `checksum` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `partial_checksum` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `temp_checksum` on the `file_paths` table. All the data in the column will be lost.
  - Added the required column `cas_id` to the `files` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `labels` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `albums` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `locations` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `spaces` table without a default value. This is not possible if the table is not empty.
  - Added the required column `client_id` to the `jobs` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `comments` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `tags` table without a default value. This is not possible if the table is not empty.

*/
-- CreateTable
CREATE TABLE "sync_events" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "data" TEXT NOT NULL,
    CONSTRAINT "sync_events_client_id_fkey" FOREIGN KEY ("client_id") REFERENCES "clients" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pixel_width" INTEGER,
    "pixel_height" INTEGER,
    "longitude" REAL,
    "latitude" REAL,
    "capture_device" TEXT,
    "duration" INTEGER,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE
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
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_files" ("date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "kind", "size_in_bytes") SELECT "date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "kind", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_integrity_checksum_key" ON "files"("integrity_checksum");
CREATE TABLE "new_labels" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_labels" ("date_created", "date_modified", "id", "name") SELECT "date_created", "date_modified", "id", "name" FROM "labels";
DROP TABLE "labels";
ALTER TABLE "new_labels" RENAME TO "labels";
CREATE UNIQUE INDEX "labels_uuid_key" ON "labels"("uuid");
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
    "permissions" TEXT,
    "temp_cas_id" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_created", "date_indexed", "date_modified", "encryption", "extension", "file_id", "id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "permissions") SELECT "date_created", "date_indexed", "date_modified", "encryption", "extension", "file_id", "id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "permissions" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");
CREATE TABLE "new_albums" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_albums" ("date_created", "date_modified", "id", "is_hidden", "name") SELECT "date_created", "date_modified", "id", "is_hidden", "name" FROM "albums";
DROP TABLE "albums";
ALTER TABLE "new_albums" RENAME TO "albums";
CREATE UNIQUE INDEX "albums_uuid_key" ON "albums"("uuid");
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "client_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN NOT NULL DEFAULT true,
    "is_ejectable" BOOLEAN NOT NULL DEFAULT true,
    "is_root_filesystem" BOOLEAN NOT NULL DEFAULT true,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_locations" ("available_capacity", "date_created", "disk_type", "filesystem", "id", "is_ejectable", "is_online", "is_removable", "is_root_filesystem", "local_path", "name", "total_capacity") SELECT "available_capacity", "date_created", "disk_type", "filesystem", "id", "is_ejectable", "is_online", "is_removable", "is_root_filesystem", "local_path", "name", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
CREATE UNIQUE INDEX "locations_uuid_key" ON "locations"("uuid");
CREATE TABLE "new_spaces" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "encryption" INTEGER DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "libraryId" INTEGER,
    CONSTRAINT "spaces_libraryId_fkey" FOREIGN KEY ("libraryId") REFERENCES "libraries" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_spaces" ("date_created", "date_modified", "encryption", "id", "libraryId", "name") SELECT "date_created", "date_modified", "encryption", "id", "libraryId", "name" FROM "spaces";
DROP TABLE "spaces";
ALTER TABLE "new_spaces" RENAME TO "spaces";
CREATE UNIQUE INDEX "spaces_uuid_key" ON "spaces"("uuid");
CREATE TABLE "new_jobs" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "client_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "seconds_elapsed" INTEGER NOT NULL DEFAULT 0
);
INSERT INTO "new_jobs" ("action", "completed_task_count", "date_created", "date_modified", "id", "seconds_elapsed", "status", "task_count") SELECT "action", "completed_task_count", "date_created", "date_modified", "id", "seconds_elapsed", "status", "task_count" FROM "jobs";
DROP TABLE "jobs";
ALTER TABLE "new_jobs" RENAME TO "jobs";
CREATE TABLE "new_comments" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "file_id" INTEGER,
    CONSTRAINT "comments_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_comments" ("content", "date_created", "date_modified", "file_id", "id") SELECT "content", "date_created", "date_modified", "file_id", "id" FROM "comments";
DROP TABLE "comments";
ALTER TABLE "new_comments" RENAME TO "comments";
CREATE UNIQUE INDEX "comments_uuid_key" ON "comments"("uuid");
CREATE TABLE "new_tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT,
    "encryption" INTEGER DEFAULT 0,
    "total_files" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_tags" ("date_created", "date_modified", "encryption", "id", "name", "redundancy_goal", "total_files") SELECT "date_created", "date_modified", "encryption", "id", "name", "redundancy_goal", "total_files" FROM "tags";
DROP TABLE "tags";
ALTER TABLE "new_tags" RENAME TO "tags";
CREATE UNIQUE INDEX "tags_uuid_key" ON "tags"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
