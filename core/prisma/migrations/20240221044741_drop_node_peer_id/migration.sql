/*
  Warnings:

  - You are about to drop the column `pub_id` on the `label` table. All the data in the column will be lost.
  - You are about to drop the column `node_peer_id` on the `node` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_label" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_label" ("date_created", "date_modified", "id", "name") SELECT "date_created", "date_modified", "id", "name" FROM "label";
DROP TABLE "label";
ALTER TABLE "new_label" RENAME TO "label";
CREATE UNIQUE INDEX "label_name_key" ON "label"("name");
CREATE TABLE "new_node" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL,
    "date_created" DATETIME NOT NULL,
    "identity" BLOB
);
INSERT INTO "new_node" ("date_created", "id", "identity", "name", "platform", "pub_id") SELECT "date_created", "id", "identity", "name", "platform", "pub_id" FROM "node";
DROP TABLE "node";
ALTER TABLE "new_node" RENAME TO "node";
CREATE UNIQUE INDEX "node_pub_id_key" ON "node"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
