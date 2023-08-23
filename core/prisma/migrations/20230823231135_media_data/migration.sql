/*
  Warnings:

  - Added the required column `object_id` to the `media_data` table without a default value. This is not possible if the table is not empty.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "dimensions" BLOB NOT NULL,
    "media_date" BLOB NOT NULL,
    "media_location" BLOB,
    "camera_data" BLOB NOT NULL,
    "artist" BLOB,
    "description" BLOB,
    "copyright" BLOB,
    "exif_version" BLOB,
    "object_id" INTEGER NOT NULL,
    CONSTRAINT "media_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("artist", "camera_data", "copyright", "dimensions", "exif_version", "id", "media_date", "media_location") SELECT "artist", "camera_data", "copyright", "dimensions", "exif_version", "id", "media_date", "media_location" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
CREATE UNIQUE INDEX "media_data_object_id_key" ON "media_data"("object_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
