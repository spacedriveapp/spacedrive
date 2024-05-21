-- AlterTable
ALTER TABLE "job" ADD COLUMN "critical_error" TEXT;
ALTER TABLE "job" ADD COLUMN "non_critical_errors" BLOB;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "size_in_bytes" BLOB,
    "is_archived" BOOLEAN,
    "generate_preview_media" BOOLEAN,
    "sync_preview_media" BOOLEAN,
    "hidden" BOOLEAN,
    "date_created" DATETIME,
    "scan_state" INTEGER NOT NULL DEFAULT 0,
    "instance_id" INTEGER,
    CONSTRAINT "location_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "size_in_bytes", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "instance_id", "is_archived", "name", "path", "pub_id", "size_in_bytes", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
