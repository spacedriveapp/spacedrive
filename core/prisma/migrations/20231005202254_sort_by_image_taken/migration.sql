/*
  Warnings:

  - You are about to drop the column `dimensions` on the `media_data` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "resolution" BLOB,
    "media_date" BLOB,
    "media_location" BLOB,
    "camera_data" BLOB,
    "artist" TEXT,
    "description" TEXT,
    "copyright" TEXT,
    "exif_version" TEXT,
    "epoch_time" BIGINT,
    "object_id" INTEGER NOT NULL,
    CONSTRAINT "media_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("artist", "camera_data", "copyright", "description", "exif_version", "id", "media_date", "media_location", "object_id") SELECT "artist", "camera_data", "copyright", "description", "exif_version", "id", "media_date", "media_location", "object_id" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
CREATE UNIQUE INDEX "media_data_object_id_key" ON "media_data"("object_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
