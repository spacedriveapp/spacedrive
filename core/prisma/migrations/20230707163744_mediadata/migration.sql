/*
  Warnings:

  - You are about to drop the column `capture_device_make` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_model` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_software` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `duration_seconds` on the `media_data` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_created" DATETIME,
    "date_taken" DATETIME,
    "pixel_width" INTEGER,
    "pixel_height" INTEGER,
    "color_space" TEXT,
    "longitude" REAL,
    "latitude" REAL,
    "codecs" TEXT,
    "device_make" TEXT,
    "device_model" TEXT,
    "device_software" TEXT,
    "focal_length" REAL,
    "shutter_speed" REAL,
    "orientation" TEXT,
    "copyright" TEXT,
    "flash" BOOLEAN,
    "duration" INTEGER,
    "fps" INTEGER,
    "streams" INTEGER,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("codecs", "fps", "id", "latitude", "longitude", "pixel_height", "pixel_width", "streams") SELECT "codecs", "fps", "id", "latitude", "longitude", "pixel_height", "pixel_width", "streams" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
