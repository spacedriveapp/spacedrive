/*
  Warnings:

  - You are about to drop the column `capture_device_make` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_model` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `capture_device_software` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `duration_seconds` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `latitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `longitude` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_height` on the `media_data` table. All the data in the column will be lost.
  - You are about to drop the column `pixel_width` on the `media_data` table. All the data in the column will be lost.
  - Added the required column `camera_data` to the `media_data` table without a default value. This is not possible if the table is not empty.
  - Added the required column `dimensions` to the `media_data` table without a default value. This is not possible if the table is not empty.
  - Added the required column `media_date` to the `media_data` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "instance" ADD COLUMN "timestamp" BIGINT;

-- CreateTable
CREATE TABLE "relation_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "relation" TEXT NOT NULL,
    "item_id" BLOB NOT NULL,
    "group_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "relation_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "dimensions" BLOB NOT NULL,
    "media_date" BLOB NOT NULL,
    "camera_data" BLOB NOT NULL,
    "location" BLOB,
    "copyright" TEXT,
    "artist" TEXT,
    "duration" INTEGER,
    "fps" INTEGER,
    "streams" INTEGER,
    "codecs" TEXT,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("codecs", "fps", "id", "streams") SELECT "codecs", "fps", "id", "streams" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
