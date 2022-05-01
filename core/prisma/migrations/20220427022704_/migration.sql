-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_library_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "library_id" INTEGER NOT NULL,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "library_db_size" TEXT NOT NULL DEFAULT '0',
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "total_bytes_free" TEXT NOT NULL DEFAULT '0',
    "preview_media_bytes" TEXT NOT NULL DEFAULT '0'
);
INSERT INTO "new_library_statistics" ("date_captured", "id", "library_id", "preview_media_bytes", "total_bytes_capacity", "total_bytes_free", "total_bytes_used", "total_file_count", "total_unique_bytes") SELECT "date_captured", "id", "library_id", "preview_media_bytes", "total_bytes_capacity", "total_bytes_free", "total_bytes_used", "total_file_count", "total_unique_bytes" FROM "library_statistics";
DROP TABLE "library_statistics";
ALTER TABLE "new_library_statistics" RENAME TO "library_statistics";
CREATE UNIQUE INDEX "library_statistics_library_id_key" ON "library_statistics"("library_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
