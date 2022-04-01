/*
  Warnings:

  - You are about to drop the column `extension` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `id_hash` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `name` on the `files` table. All the data in the column will be lost.
  - Added the required column `partial_checksum` to the `files` table without a default value. This is not possible if the table is not empty.
  - Added the required column `name` to the `file_paths` table without a default value. This is not possible if the table is not empty.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "type" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
    "partial_checksum" TEXT NOT NULL,
    "checksum" TEXT,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "ipfs_id" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_files" ("checksum", "date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "size_in_bytes") SELECT "checksum", "date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_checksum_key" ON "files"("checksum");
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
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_indexed", "file_id", "id", "is_dir", "location_id", "materialized_path", "parent_id", "permissions") SELECT "date_indexed", "file_id", "id", "is_dir", "location_id", "materialized_path", "parent_id", "permissions" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
