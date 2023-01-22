/*
  Warnings:

  - You are about to drop the `sync_event` table. If the table is not empty, all the data it contains will be lost.
  - Added the required column `position` to the `tag` table without a default value. This is not possible if the table is not empty.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "sync_event";
PRAGMA foreign_keys=on;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_tag" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "color" TEXT,
    "total_objects" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "position" INTEGER NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_tag" ("color", "date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_objects") SELECT "color", "date_created", "date_modified", "id", "name", "pub_id", "redundancy_goal", "total_objects" FROM "tag";
DROP TABLE "tag";
ALTER TABLE "new_tag" RENAME TO "tag";
CREATE UNIQUE INDEX "tag_pub_id_key" ON "tag"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
