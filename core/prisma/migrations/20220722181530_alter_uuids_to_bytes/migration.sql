/*
  Warnings:

  - The primary key for the `jobs` table will be changed. If it partially fails, the table could be left without primary key constraint.
  - You are about to alter the column `data` on the `jobs` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `id` on the `jobs` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `nodes` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `tags` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `labels` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `spaces` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `locations` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `record_id` on the `sync_events` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `albums` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `pub_id` on the `comments` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_jobs" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "data" BLOB,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "seconds_elapsed" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "jobs_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_jobs" ("action", "completed_task_count", "data", "date_created", "date_modified", "id", "name", "node_id", "seconds_elapsed", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_created", "date_modified", "id", "name", "node_id", "seconds_elapsed", "status", "task_count" FROM "jobs";
DROP TABLE "jobs";
ALTER TABLE "new_jobs" RENAME TO "jobs";
CREATE TABLE "new_nodes" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL DEFAULT 0,
    "version" TEXT,
    "last_seen" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_nodes" ("date_created", "id", "last_seen", "name", "platform", "pub_id", "timezone", "version") SELECT "date_created", "id", "last_seen", "name", "platform", "pub_id", "timezone", "version" FROM "nodes";
DROP TABLE "nodes";
ALTER TABLE "new_nodes" RENAME TO "nodes";
CREATE UNIQUE INDEX "nodes_pub_id_key" ON "nodes"("pub_id");
CREATE TABLE "new_tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "total_files" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_tags" ("color", "date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_files") SELECT "color", "date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_files" FROM "tags";
DROP TABLE "tags";
ALTER TABLE "new_tags" RENAME TO "tags";
CREATE UNIQUE INDEX "tags_pub_id_key" ON "tags"("pub_id");
CREATE TABLE "new_labels" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_labels" ("date_created", "date_modified", "id", "name", "pub_id") SELECT "date_created", "date_modified", "id", "name", "pub_id" FROM "labels";
DROP TABLE "labels";
ALTER TABLE "new_labels" RENAME TO "labels";
CREATE UNIQUE INDEX "labels_pub_id_key" ON "labels"("pub_id");
CREATE TABLE "new_spaces" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "description" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_spaces" ("date_created", "date_modified", "description", "id", "name", "pub_id") SELECT "date_created", "date_modified", "description", "id", "name", "pub_id" FROM "spaces";
DROP TABLE "spaces";
ALTER TABLE "new_spaces" RENAME TO "spaces";
CREATE UNIQUE INDEX "spaces_pub_id_key" ON "spaces"("pub_id");
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "locations_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_locations" ("available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "node_id", "pub_id", "total_capacity") SELECT "available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "node_id", "pub_id", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");
CREATE TABLE "new_sync_events" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "node_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" INTEGER NOT NULL,
    "column" TEXT,
    "value" TEXT NOT NULL,
    CONSTRAINT "sync_events_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_sync_events" ("column", "id", "kind", "node_id", "record_id", "timestamp", "value") SELECT "column", "id", "kind", "node_id", "record_id", "timestamp", "value" FROM "sync_events";
DROP TABLE "sync_events";
ALTER TABLE "new_sync_events" RENAME TO "sync_events";
CREATE TABLE "new_albums" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_albums" ("date_created", "date_modified", "id", "is_hidden", "name", "pub_id") SELECT "date_created", "date_modified", "id", "is_hidden", "name", "pub_id" FROM "albums";
DROP TABLE "albums";
ALTER TABLE "new_albums" RENAME TO "albums";
CREATE UNIQUE INDEX "albums_pub_id_key" ON "albums"("pub_id");
CREATE TABLE "new_comments" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "file_id" INTEGER,
    CONSTRAINT "comments_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_comments" ("content", "date_created", "date_modified", "file_id", "id", "pub_id") SELECT "content", "date_created", "date_modified", "file_id", "id", "pub_id" FROM "comments";
DROP TABLE "comments";
ALTER TABLE "new_comments" RENAME TO "comments";
CREATE UNIQUE INDEX "comments_pub_id_key" ON "comments"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
