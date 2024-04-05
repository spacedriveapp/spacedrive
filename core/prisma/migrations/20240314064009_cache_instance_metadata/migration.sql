/*
  Warnings:

  - You are about to drop the column `node_name` on the `instance` table. All the data in the column will be lost.
  - You are about to drop the column `node_platform` on the `instance` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "identity" BLOB NOT NULL,
    "node_id" BLOB NOT NULL,
    "metadata" BLOB,
    "last_seen" DATETIME NOT NULL,
    "date_created" DATETIME NOT NULL,
    "timestamp" BIGINT
);
INSERT INTO "new_instance" ("date_created", "id", "identity", "last_seen", "node_id", "pub_id", "timestamp") SELECT "date_created", "id", "identity", "last_seen", "node_id", "pub_id", "timestamp" FROM "instance";
DROP TABLE "instance";
ALTER TABLE "new_instance" RENAME TO "instance";
CREATE UNIQUE INDEX "instance_pub_id_key" ON "instance"("pub_id");
CREATE TABLE "new_label" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "date_created" DATETIME,
    "date_modified" DATETIME
);
INSERT INTO "new_label" ("date_created", "date_modified", "id", "name") SELECT "date_created", "date_modified", "id", "name" FROM "label";
DROP TABLE "label";
ALTER TABLE "new_label" RENAME TO "label";
CREATE UNIQUE INDEX "label_name_key" ON "label"("name");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
