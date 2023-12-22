/*
  Warnings:

  - You are about to drop the `relation_operation` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the `shared_operation` table. If the table is not empty, all the data it contains will be lost.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "relation_operation";
PRAGMA foreign_keys=on;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "shared_operation";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "crdt_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "crdt_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "cloud_crdt_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "cloud_crdt_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
