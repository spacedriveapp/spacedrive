/*
  Warnings:

  - You are about to drop the column `full_checksum` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `is_dir` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `location_id` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `parent_id` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `quick_checksum` on the `files` table. All the data in the column will be lost.
  - You are about to drop the column `stem` on the `files` table. All the data in the column will be lost.
  - Added the required column `file_hash` to the `files` table without a default value. This is not possible if the table is not empty.

*/
-- CreateTable
CREATE TABLE "file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "materialized_path" TEXT NOT NULL,
    "file_id" INTEGER,
    "logical_file_parent_id" INTEGER,
    "location_id" INTEGER NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "permissions" TEXT,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_logical_file_parent_id_fkey" FOREIGN KEY ("logical_file_parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "labels" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "label_on_files" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "label_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("label_id", "file_id"),
    CONSTRAINT "label_on_files_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "label_on_files_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "labels" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "albums" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "comments" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "file_id" INTEGER,
    CONSTRAINT "comments_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "file_hash" TEXT NOT NULL,
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
INSERT INTO "new_files" ("date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "name", "size_in_bytes") SELECT "date_created", "date_indexed", "date_modified", "encryption", "extension", "id", "ipfs_id", "name", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_file_hash_key" ON "files"("file_hash");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_file_id_key" ON "file_paths"("location_id", "materialized_path", "file_id");
