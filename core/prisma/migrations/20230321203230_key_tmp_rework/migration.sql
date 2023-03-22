/*
  Warnings:

  - You are about to drop the column `local_path` on the `location` table. All the data in the column will be lost.
  - You are about to drop the column `automount` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `default` on the `key` table. All the data in the column will be lost.
  - You are about to alter the column `algorithm` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `hashing_algorithm` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `key_type` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `name` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `uuid` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `version` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - Added the required column `path` to the `location` table without a default value. This is not possible if the table is not empty.
  - Made the column `name` on table `location` required. This step will fail if there are existing NULL values in that column.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "path" TEXT NOT NULL,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_archived" BOOLEAN NOT NULL DEFAULT false,
    "generate_preview_media" BOOLEAN NOT NULL DEFAULT true,
    "sync_preview_media" BOOLEAN NOT NULL DEFAULT true,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "location_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "node_id", "pub_id", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "node_id", "pub_id", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
CREATE TABLE "new_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" BLOB NOT NULL,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "version" BLOB NOT NULL,
    "key_type" BLOB NOT NULL,
    "name" BLOB,
    "algorithm" BLOB NOT NULL,
    "hashing_algorithm" BLOB NOT NULL,
    "content_salt" BLOB NOT NULL,
    "master_key" BLOB NOT NULL,
    "master_key_nonce" BLOB NOT NULL,
    "key_nonce" BLOB NOT NULL,
    "key" BLOB NOT NULL,
    "salt" BLOB NOT NULL
);
INSERT INTO "new_key" ("algorithm", "content_salt", "date_created", "hashing_algorithm", "id", "key", "key_nonce", "key_type", "master_key", "master_key_nonce", "name", "salt", "uuid", "version") SELECT "algorithm", "content_salt", "date_created", "hashing_algorithm", "id", "key", "key_nonce", "key_type", "master_key", "master_key_nonce", "name", "salt", "uuid", "version" FROM "key";
DROP TABLE "key";
ALTER TABLE "new_key" RENAME TO "key";
CREATE UNIQUE INDEX "key_uuid_key" ON "key"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
