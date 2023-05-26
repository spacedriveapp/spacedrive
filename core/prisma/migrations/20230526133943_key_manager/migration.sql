/*
  Warnings:

  - You are about to drop the column `automount` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `content_salt` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `default` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `key_nonce` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `master_key` on the `key` table. All the data in the column will be lost.
  - You are about to drop the column `master_key_nonce` on the `key` table. All the data in the column will be lost.
  - You are about to alter the column `algorithm` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `hashing_algorithm` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `key_type` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Int`.
  - You are about to alter the column `uuid` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Binary`.
  - You are about to alter the column `version` on the `key` table. The data in that column could be lost. The data in that column will be cast from `String` to `Int`.

*/
-- CreateTable
CREATE TABLE "mounted_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" BLOB NOT NULL,
    "version" INTEGER NOT NULL,
    "algorithm" BLOB NOT NULL,
    "key" BLOB NOT NULL,
    "salt" BLOB NOT NULL,
    CONSTRAINT "mounted_key_uuid_fkey" FOREIGN KEY ("uuid") REFERENCES "key" ("uuid") ON DELETE CASCADE ON UPDATE NO ACTION
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" BLOB NOT NULL,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "version" INTEGER NOT NULL,
    "key_type" INTEGER NOT NULL,
    "name" TEXT,
    "algorithm" BLOB NOT NULL,
    "hashing_algorithm" BLOB NOT NULL,
    "key" BLOB NOT NULL,
    "salt" BLOB NOT NULL
);
INSERT INTO "new_key" ("algorithm", "date_created", "hashing_algorithm", "id", "key", "key_type", "name", "salt", "uuid", "version") SELECT "algorithm", "date_created", "hashing_algorithm", "id", "key", "key_type", "name", "salt", "uuid", "version" FROM "key";
DROP TABLE "key";
ALTER TABLE "new_key" RENAME TO "key";
CREATE UNIQUE INDEX "key_uuid_key" ON "key"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "mounted_key_uuid_key" ON "mounted_key"("uuid");
