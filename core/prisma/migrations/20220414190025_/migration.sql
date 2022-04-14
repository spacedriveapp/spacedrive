/*
  Warnings:

  - You are about to drop the column `capture_device` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `duration` on the `media_data` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pixel_width" INTEGER,
    "pixel_height" INTEGER,
    "longitude" REAL,
    "latitude" REAL,
    "fps" INTEGER,
    "capture_device_make" TEXT,
    "capture_device_model" TEXT,
    "capture_device_software" TEXT,
    "duration_seconds" INTEGER,
    "codecs" TEXT,
    "streams_json" TEXT,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("id", "latitude", "longitude", "pixel_height", "pixel_width") SELECT "id", "latitude", "longitude", "pixel_height", "pixel_width" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
