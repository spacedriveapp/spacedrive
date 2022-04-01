/*
  Warnings:

  - You are about to drop the column `full_integrity_hash` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `path_integrity_hash` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `quick_integrity_hash` on the `files` table. All the data in the column will be lost.
  - Added the required column `path_checksum` to the `files` table without a default value. This is not possible if the table is not empty.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "path_checksum" TEXT NOT NULL,
    "quick_checksum" TEXT,
    "full_checksum" TEXT,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "ipfs_id" TEXT,
    "parent_id" INTEGER,
    CONSTRAINT "files_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "files_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_files" ("date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "size_in_bytes") SELECT "date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "is_dir", "location_id", "materialized_path", "name", "parent_id", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_path_checksum_key" ON "files"("path_checksum");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
