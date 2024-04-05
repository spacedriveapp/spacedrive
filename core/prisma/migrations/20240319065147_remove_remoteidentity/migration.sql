/*
  Warnings:

  - The primary key for the `cloud_crdt_operation` table will be changed. If it partially fails, the table could be left without primary key constraint.
  - You are about to alter the column `id` on the `cloud_crdt_operation` table. The data in that column could be lost. The data in that column will be cast from `Binary` to `Int`.
  - The primary key for the `crdt_operation` table will be changed. If it partially fails, the table could be left without primary key constraint.
  - You are about to alter the column `id` on the `crdt_operation` table. The data in that column could be lost. The data in that column will be cast from `Binary` to `Int`.
  - Added the required column `remote_identity` to the `instance` table without a default value. This is not possible if the table is not empty.

 - @oscartbeaumont modified the migration Prisma generated to fill the `NOT NULL` `remote_identity` field with the existing IdentityOrRemoteIdentity value so we can handle it in the app migrations.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_cloud_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "cloud_crdt_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_cloud_crdt_operation" ("data", "id", "instance_id", "kind", "model", "record_id", "timestamp") SELECT "data", "id", "instance_id", "kind", "model", "record_id", "timestamp" FROM "cloud_crdt_operation";
DROP TABLE "cloud_crdt_operation";
ALTER TABLE "new_cloud_crdt_operation" RENAME TO "cloud_crdt_operation";
CREATE TABLE "new_instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "identity" BLOB,
    "remote_identity" BLOB NOT NULL,
    "node_id" BLOB NOT NULL,
    "metadata" BLOB,
    "last_seen" DATETIME NOT NULL,
    "date_created" DATETIME NOT NULL,
    "timestamp" BIGINT
);
INSERT INTO "new_instance" ("date_created", "id", "identity", "remote_identity", "last_seen", "metadata", "node_id", "pub_id", "timestamp") SELECT "date_created", "id", "identity", "identity", "last_seen", "metadata", "node_id", "pub_id", "timestamp" FROM "instance";
DROP TABLE "instance";
ALTER TABLE "new_instance" RENAME TO "instance";
CREATE UNIQUE INDEX "instance_pub_id_key" ON "instance"("pub_id");
CREATE TABLE "new_crdt_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
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
