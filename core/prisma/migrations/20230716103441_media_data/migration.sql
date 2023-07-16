/*
  Warnings:

  - You are about to drop the column `device_software` on the `media_data` table. All the data in the column will be lost.

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
    "focal_length" REAL,
    "shutter_speed" REAL,
    "orientation" TEXT,
    "flash" BOOLEAN,
    "copyright" TEXT,
    "artist" TEXT,
    "duration" INTEGER,
    "fps" INTEGER,
    "streams" INTEGER,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("codecs", "color_space", "copyright", "date_created", "date_taken", "device_make", "device_model", "duration", "flash", "focal_length", "fps", "id", "latitude", "longitude", "orientation", "pixel_height", "pixel_width", "shutter_speed", "streams") SELECT "codecs", "color_space", "copyright", "date_created", "date_taken", "device_make", "device_model", "duration", "flash", "focal_length", "fps", "id", "latitude", "longitude", "orientation", "pixel_height", "pixel_width", "shutter_speed", "streams" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
