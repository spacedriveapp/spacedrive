/*
  Warnings:

  - You are about to drop the column `redundancy_goal` on the `tag` table. All the data in the column will be lost.
  - Made the column `name` on table `label` required. This step will fail if there are existing NULL values in that column.

*/
-- AlterTable
ALTER TABLE "saved_search" ADD COLUMN "search" TEXT;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_label" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_label" ("date_created", "date_modified", "id", "name", "pub_id") SELECT "date_created", "date_modified", "id", "name", "pub_id" FROM "label";
DROP TABLE "label";
ALTER TABLE "new_label" RENAME TO "label";
CREATE UNIQUE INDEX "label_pub_id_key" ON "label"("pub_id");
CREATE UNIQUE INDEX "label_name_key" ON "label"("name");
CREATE TABLE "new_tag" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "is_hidden" BOOLEAN,
    "date_created" DATETIME,
    "date_modified" DATETIME
);
INSERT INTO "new_tag" ("color", "date_created", "date_modified", "id", "name", "pub_id") SELECT "color", "date_created", "date_modified", "id", "name", "pub_id" FROM "tag";
DROP TABLE "tag";
ALTER TABLE "new_tag" RENAME TO "tag";
CREATE UNIQUE INDEX "tag_pub_id_key" ON "tag"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
