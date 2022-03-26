/*
  Warnings:

  - You are about to drop the column `path` on the `locations` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN NOT NULL DEFAULT true,
    "is_ejectable" BOOLEAN NOT NULL DEFAULT true,
    "is_root_filesystem" BOOLEAN NOT NULL DEFAULT true,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_locations" ("available_capacity", "date_created", "disk_type", "filesystem", "id", "is_ejectable", "is_online", "is_removable", "is_root_filesystem", "name", "total_capacity") SELECT "available_capacity", "date_created", "disk_type", "filesystem", "id", "is_ejectable", "is_online", "is_removable", "is_root_filesystem", "name", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
