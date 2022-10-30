/*
  Warnings:

  - You are about to drop the column `checksum` on the `key` table. All the data in the column will be lost.
  - You are about to alter the column `algorithm` on the `key` table. The data in that column could be lost. The data in that column will be cast from `Int` to `Binary`.
  - Added the required column `content_salt` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `default` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `hashing_algorithm` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `key` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `key_nonce` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `master_key` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `master_key_nonce` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `salt` to the `key` table without a default value. This is not possible if the table is not empty.
  - Added the required column `uuid` to the `key` table without a default value. This is not possible if the table is not empty.
  - Made the column `algorithm` on table `key` required. This step will fail if there are existing NULL values in that column.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT,
    "default" BOOLEAN NOT NULL,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "algorithm" BLOB NOT NULL,
    "hashing_algorithm" BLOB NOT NULL,
    "salt" BLOB NOT NULL,
    "content_salt" BLOB NOT NULL,
    "master_key" BLOB NOT NULL,
    "master_key_nonce" BLOB NOT NULL,
    "key_nonce" BLOB NOT NULL,
    "key" BLOB NOT NULL
);
INSERT INTO "new_key" ("algorithm", "date_created", "id", "name") SELECT "algorithm", "date_created", "id", "name" FROM "key";
DROP TABLE "key";
ALTER TABLE "new_key" RENAME TO "key";
CREATE UNIQUE INDEX "key_uuid_key" ON "key"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
