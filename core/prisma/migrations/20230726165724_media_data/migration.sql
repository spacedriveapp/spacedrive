/*
  Warnings:

  - You are about to drop the column `color_space` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `date_created` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `date_taken` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `device_make` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `device_model` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `device_software` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `flash` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `focal_length` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `latitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `longitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `orientation` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_height` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_width` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `shutter_speed` on the `media_data` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "dimensions" BLOB,
    "image_date" BLOB,
    "location" BLOB,
    "camera_data" BLOB,
    "software" TEXT,
    "copyright" TEXT,
    "artist" TEXT,
    "duration" INTEGER,
    "fps" INTEGER,
    "streams" INTEGER,
    "codecs" TEXT,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("codecs", "copyright", "duration", "fps", "id", "streams") SELECT "codecs", "copyright", "duration", "fps", "id", "streams" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
