-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_archived" BOOLEAN,
    "generate_preview_media" BOOLEAN,
    "sync_preview_media" BOOLEAN,
    "hidden" BOOLEAN,
    "date_created" DATETIME,
    CONSTRAINT "location_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_location" ("available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "node_id", "path", "pub_id", "sync_preview_media", "total_capacity") SELECT "available_capacity", "date_created", "generate_preview_media", "hidden", "id", "is_archived", "name", "node_id", "path", "pub_id", "sync_preview_media", "total_capacity" FROM "location";
DROP TABLE "location";
ALTER TABLE "new_location" RENAME TO "location";
CREATE UNIQUE INDEX "location_pub_id_key" ON "location"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
