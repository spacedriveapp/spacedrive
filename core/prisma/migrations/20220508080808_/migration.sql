/*
  Warnings:

  - You are about to drop the `spaces` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `encryption` on the `libraries` table. All the data in the column will be lost.
  - You are about to drop the column `encryption` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `encryption` on the `file_paths` table. All the data in the column will be lost.
  - You are about to drop the column `permissions` on the `file_paths` table. All the data in the column will be lost.
  - You are about to drop the column `temp_cas_id` on the `file_paths` table. All the data in the column will be lost.

*/
-- DropIndex
DROP INDEX "spaces_pub_id_key";

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "spaces";
PRAGMA foreign_keys=on;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_libraries" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "remote_id" TEXT,
    "is_primary" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT
);
INSERT INTO "new_libraries" ("date_created", "id", "is_primary", "name", "pub_id", "remote_id", "timezone") SELECT "date_created", "id", "is_primary", "name", "pub_id", "remote_id", "timezone" FROM "libraries";
DROP TABLE "libraries";
ALTER TABLE "new_libraries" RENAME TO "libraries";
CREATE UNIQUE INDEX "libraries_pub_id_key" ON "libraries"("pub_id");
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
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
INSERT INTO "new_files" ("cas_id", "comment", "date_created", "date_indexed", "date_modified", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "key_id", "kind", "size_in_bytes") SELECT "cas_id", "comment", "date_created", "date_indexed", "date_modified", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "key_id", "kind", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_cas_id_key" ON "files"("cas_id");
CREATE UNIQUE INDEX "files_integrity_checksum_key" ON "files"("integrity_checksum");
CREATE TABLE "new_file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "file_paths_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_created", "date_indexed", "date_modified", "extension", "file_id", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "parent_id") SELECT "date_created", "date_indexed", "date_modified", "extension", "file_id", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "parent_id" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
