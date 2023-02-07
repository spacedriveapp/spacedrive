/*
  Warnings:

  - You are about to drop the `sync_event` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `cas_id` on the `object` table. All the data in the column will be lost.
  - You are about to drop the column `integrity_checksum` on the `object` table. All the data in the column will be lost.
  - A unique constraint covering the columns `[integrity_checksum]` on the table `file_path` will be added. If there are existing duplicate values, this will fail.
  - Added the required column `key_type` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `pub_id` to the `object` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "file_path" ADD COLUMN "cas_id" TEXT;
ALTER TABLE "file_path" ADD COLUMN "integrity_checksum" TEXT;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "sync_event";
PRAGMA foreign_keys=on;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "version" TEXT NOT NULL,
    "key_type" TEXT NOT NULL,
    "name" TEXT,
    "default" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "algorithm" TEXT NOT NULL,
    "hashing_algorithm" TEXT NOT NULL,
    "content_salt" BLOB NOT NULL,
    "master_key" BLOB NOT NULL,
    "master_key_nonce" BLOB NOT NULL,
    "key_nonce" BLOB NOT NULL,
    "key" BLOB NOT NULL,
    "salt" BLOB NOT NULL,
    "automount" BOOLEAN NOT NULL DEFAULT false
);
INSERT INTO "new_key" ("algorithm", "automount", "content_salt", "date_created", "default", "hashing_algorithm", "id", "key", "key_nonce", "master_key", "master_key_nonce", "name", "salt", "uuid", "version") SELECT "algorithm", "automount", "content_salt", "date_created", "default", "hashing_algorithm", "id", "key", "key_nonce", "master_key", "master_key_nonce", "name", "salt", "uuid", "version" FROM "key";
DROP TABLE "key";
ALTER TABLE "new_key" RENAME TO "key";
CREATE UNIQUE INDEX "key_uuid_key" ON "key"("uuid");
CREATE TABLE "new_object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "extension" TEXT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL DEFAULT '0',
    "key_id" INTEGER,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbnail" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "note" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "object_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_object" ("date_created", "date_indexed", "date_modified", "extension", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "ipfs_id", "key_id", "kind", "name", "note", "size_in_bytes") SELECT "date_created", "date_indexed", "date_modified", "extension", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "ipfs_id", "key_id", "kind", "name", "note", "size_in_bytes" FROM "object";
DROP TABLE "object";
ALTER TABLE "new_object" RENAME TO "object";
CREATE UNIQUE INDEX "object_pub_id_key" ON "object"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "file_path_integrity_checksum_key" ON "file_path"("integrity_checksum");
