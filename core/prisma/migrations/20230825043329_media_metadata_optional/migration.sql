-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "dimensions" BLOB,
    "media_date" BLOB,
    "media_location" BLOB,
    "camera_data" BLOB,
    "artist" BLOB,
    "description" BLOB,
    "copyright" BLOB,
    "exif_version" BLOB,
    "object_id" INTEGER NOT NULL,
    CONSTRAINT "media_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_media_data" ("artist", "camera_data", "copyright", "description", "dimensions", "exif_version", "id", "media_date", "media_location", "object_id") SELECT "artist", "camera_data", "copyright", "description", "dimensions", "exif_version", "id", "media_date", "media_location", "object_id" FROM "media_data";
DROP TABLE "media_data";
ALTER TABLE "new_media_data" RENAME TO "media_data";
CREATE UNIQUE INDEX "media_data_object_id_key" ON "media_data"("object_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
