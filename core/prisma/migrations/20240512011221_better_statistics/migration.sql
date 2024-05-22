/*
  Warnings:

  - You are about to drop the column `preview_media_bytes` on the `statistics` table. All the data in the column will be lost.
  - You are about to drop the column `total_bytes_capacity` on the `statistics` table. All the data in the column will be lost.
  - You are about to drop the column `total_bytes_free` on the `statistics` table. All the data in the column will be lost.
  - You are about to drop the column `total_bytes_used` on the `statistics` table. All the data in the column will be lost.
  - You are about to drop the column `total_unique_bytes` on the `statistics` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "total_object_count" INTEGER NOT NULL DEFAULT 0,
    "library_db_size" TEXT NOT NULL DEFAULT '0',
    "total_local_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_local_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_local_bytes_free" TEXT NOT NULL DEFAULT '0',
    "total_library_bytes" TEXT NOT NULL DEFAULT '0',
    "total_library_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "total_library_preview_media_bytes" TEXT NOT NULL DEFAULT '0'
);
INSERT INTO "new_statistics" ("date_captured", "id", "library_db_size", "total_object_count") SELECT "date_captured", "id", "library_db_size", "total_object_count" FROM "statistics";
DROP TABLE "statistics";
ALTER TABLE "new_statistics" RENAME TO "statistics";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
