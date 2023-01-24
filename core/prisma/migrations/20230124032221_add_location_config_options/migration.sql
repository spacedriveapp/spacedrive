/*
  Warnings:

  - You are about to drop the column `disk_type` on the `location` table. All the data in the column will be lost.
  - You are about to drop the column `filesystem` on the `location` table. All the data in the column will be lost.
  - You are about to drop the column `is_removable` on the `location` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "is_archived" BOOLEAN NOT NULL DEFAULT false,
    "generate_preview_media" BOOLEAN NOT NULL DEFAULT true,
    "sync_preview_media" BOOLEAN NOT NULL DEFAULT true,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "location_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "id", "is_archived", "is_online", "local_path", "name", "node_id", "pub_id", "total_capacity") SELECT "available_capacity", "date_created", "id", "is_archived", "is_online", "local_path", "name", "node_id", "pub_id", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
CREATE TABLE "new_file_path" (
    "id" INTEGER NOT NULL,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "object_id" INTEGER,
    "parent_id" INTEGER,
    "key_id" INTEGER,
    "pending" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY ("location_id", "id"),
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "file_path_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_path" ("date_created", "date_indexed", "date_modified", "extension", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "parent_id", "pending") SELECT "date_created", "date_indexed", "date_modified", "extension", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "parent_id", coalesce("pending", false) AS "pending" FROM "file_path";
DROP TABLE "file_path";
ALTER TABLE "new_file_path" RENAME TO "file_path";
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
