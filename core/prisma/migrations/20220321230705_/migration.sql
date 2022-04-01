/*
  Warnings:

  - You are about to drop the column `file_hash` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `date_created` on the `file_paths` table. All the data in the column will be lost.
  - You are about to drop the column `date_modified` on the `file_paths` table. All the data in the column will be lost.
  - Added the required column `id_hash` to the `files` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "locations" ADD COLUMN "disk_type" INTEGER;
ALTER TABLE "locations" ADD COLUMN "filesystem" TEXT;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "id_hash" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "checksum" TEXT,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "ipfs_id" TEXT
);
INSERT INTO "new_files" ("checksum", "date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "name", "size_in_bytes") SELECT "checksum", "date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "name", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_id_hash_key" ON "files"("id_hash");
CREATE TABLE "new_file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "materialized_path" TEXT NOT NULL,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "location_id" INTEGER NOT NULL,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "permissions" TEXT,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_indexed", "file_id", "id", "is_dir", "location_id", "materialized_path", "parent_id", "permissions") SELECT "date_indexed", "file_id", "id", "is_dir", "location_id", "materialized_path", "parent_id", "permissions" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_file_id_key" ON "file_paths"("location_id", "materialized_path", "file_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
