/*
  Warnings:

  - You are about to drop the column `is_ejectable` on the `locations` table. All the data in the column will be lost.
  - You are about to drop the column `is_root_filesystem` on the `locations` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "client_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_locations" ("available_capacity", "client_id", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "pub_id", "total_capacity") SELECT "available_capacity", "client_id", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "pub_id", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
