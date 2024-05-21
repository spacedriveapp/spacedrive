-- CreateTable
CREATE TABLE "exif_data" (
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
        CONSTRAINT "exif_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CopyData
INSERT INTO "exif_data" (
        "id",
        "resolution",
        "media_date",
        "media_location",
        "camera_data",
        "artist",
        "description",
        "copyright",
        "exif_version",
        "epoch_time",
        "object_id"
)
SELECT
        "id",
        "resolution",
        "media_date",
        "media_location",
        "camera_data",
        "artist",
        "description",
        "copyright",
        "exif_version",
        "epoch_time",
        "object_id"
FROM
        "media_data";

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "media_data";
PRAGMA foreign_keys=on;

-- CreateIndex
CREATE UNIQUE INDEX "exif_data_object_id_key" ON "exif_data"("object_id");
