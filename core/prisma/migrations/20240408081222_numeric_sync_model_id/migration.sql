/*
  Warnings:

  - You are about to alter the column `model` on the `cloud_crdt_operation` table. The data in that column could be lost. The data in that column will be cast from `String` to `Int`.
  - You are about to alter the column `model` on the `crdt_operation` table. The data in that column could be lost. The data in that column will be cast from `String` to `Int`.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_cloud_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" INTEGER NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "cloud_crdt_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_cloud_crdt_operation" ("data", "id", "instance_id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "instance_id", "kind", "model", "record_id", "timestamp" FROM "cloud_crdt_operation";
DROP TABLE "cloud_crdt_operation";
ALTER TABLE "new_cloud_crdt_operation" RENAME TO "cloud_crdt_operation";
CREATE TABLE "new_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" INTEGER NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "crdt_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_crdt_operation" ("data", "id", "instance_id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "instance_id", "kind", "model", "record_id", "timestamp" FROM "crdt_operation";
DROP TABLE "crdt_operation";
ALTER TABLE "new_crdt_operation" RENAME TO "crdt_operation";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
