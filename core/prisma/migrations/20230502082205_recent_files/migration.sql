/*
  Warnings:

  - The primary key for the `file_path` table will be changed. If it partially fails, the table could be left without primary key constraint.
  - You are about to alter the column `parent_id` on the `file_path` table. The data in that column could be lost. The data in that column will be cast from `Int` to `Binary`.
  - Added the required column `pub_id` to the `file_path` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "object" ADD COLUMN "date_accessed" DATETIME;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_file_path" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "cas_id" TEXT,
    "integrity_checksum" TEXT,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT NOT NULL,
    "size_in_bytes" TEXT NOT NULL DEFAULT '0',
    "inode" BLOB NOT NULL,
    "device" BLOB NOT NULL,
    "object_id" INTEGER,
    "parent_id" BLOB,
    "key_id" INTEGER,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "file_path_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_path" ("cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "parent_id", "size_in_bytes") SELECT "cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "parent_id", "size_in_bytes" FROM "file_path";
DROP TABLE "file_path";
ALTER TABLE "new_file_path" RENAME TO "file_path";
CREATE UNIQUE INDEX "file_path_pub_id_key" ON "file_path"("pub_id");
CREATE UNIQUE INDEX "file_path_integrity_checksum_key" ON "file_path"("integrity_checksum");
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");
CREATE UNIQUE INDEX "file_path_location_id_inode_device_key" ON "file_path"("location_id", "inode", "device");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
