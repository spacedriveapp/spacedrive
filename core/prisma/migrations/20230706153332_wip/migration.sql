/*
  Warnings:

  - You are about to drop the column `node_id` on the `volume` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "is_system" BOOLEAN NOT NULL DEFAULT false,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_volume" ("date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "volume";
DROP TABLE "volume";
ALTER TABLE "new_volume" RENAME TO "volume";
CREATE UNIQUE INDEX "volume_mount_point_name_key" ON "volume"("mount_point", "name");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
