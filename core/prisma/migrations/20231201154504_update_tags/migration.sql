/*
  Warnings:

  - You are about to drop the `label` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the `label_on_object` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `redundancy_goal` on the `tag` table. All the data in the column will be lost.
  - Added the required column `kind` to the `tag` table without a default value. This is not possible if the table is not empty.
  - Made the column `name` on table `tag` required. This step will fail if there are existing NULL values in that column.

*/
-- DropIndex
DROP INDEX "label_pub_id_key";

-- AlterTable
ALTER TABLE "saved_search" ADD COLUMN "search" TEXT;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "label";
PRAGMA foreign_keys=on;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "label_on_object";
PRAGMA foreign_keys=on;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_tag" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "kind" INTEGER,
    "is_hidden" BOOLEAN,
    "date_created" DATETIME,
    "date_modified" DATETIME
);
INSERT INTO "new_tag" ("color", "date_created", "date_modified", "id", "name", "pub_id") SELECT "color", "date_created", "date_modified", "id", "name", "pub_id" FROM "tag";
DROP TABLE "tag";
ALTER TABLE "new_tag" RENAME TO "tag";
CREATE UNIQUE INDEX "tag_pub_id_key" ON "tag"("pub_id");
CREATE UNIQUE INDEX "tag_name_kind_key" ON "tag"("name", "kind");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
