/*
  Warnings:

  - Added the required column `timestamp` to the `instance` table without a default value. This is not possible if the table is not empty.

*/
-- CreateTable
CREATE TABLE "relation_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "relation" TEXT NOT NULL,
    "item_id" BLOB NOT NULL,
    "group_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "relation_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "identity" BLOB NOT NULL,
    "node_id" BLOB NOT NULL,
    "node_name" TEXT NOT NULL,
    "node_platform" INTEGER NOT NULL,
    "last_seen" DATETIME NOT NULL,
    "date_created" DATETIME NOT NULL,
    "timestamp" BIGINT NOT NULL
);
INSERT INTO "new_instance" ("date_created", "id", "identity", "last_seen", "node_id", "node_name", "node_platform", "pub_id") SELECT "date_created", "id", "identity", "last_seen", "node_id", "node_name", "node_platform", "pub_id" FROM "instance";
DROP TABLE "instance";
ALTER TABLE "new_instance" RENAME TO "instance";
CREATE UNIQUE INDEX "instance_pub_id_key" ON "instance"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
