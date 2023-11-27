/*
  Warnings:

  - You are about to drop the column `order` on the `saved_search` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_saved_search" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "filters" TEXT,
    "name" TEXT,
    "icon" TEXT,
    "description" TEXT,
    "date_created" DATETIME,
    "date_modified" DATETIME
);
INSERT INTO "new_saved_search" ("date_created", "date_modified", "description", "filters", "icon", "id", "name", "pub_id") SELECT "date_created", "date_modified", "description", "filters", "icon", "id", "name", "pub_id" FROM "saved_search";
DROP TABLE "saved_search";
ALTER TABLE "new_saved_search" RENAME TO "saved_search";
CREATE UNIQUE INDEX "saved_search_pub_id_key" ON "saved_search"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
