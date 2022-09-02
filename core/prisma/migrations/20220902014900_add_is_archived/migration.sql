-- AlterTable
ALTER TABLE "files" ADD COLUMN "extension" TEXT;
ALTER TABLE "files" ADD COLUMN "name" TEXT;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "node_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "is_archived" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "locations_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_locations" ("available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "node_id", "pub_id", "total_capacity") SELECT "available_capacity", "date_created", "disk_type", "filesystem", "id", "is_online", "is_removable", "local_path", "name", "node_id", "pub_id", "total_capacity" FROM "locations";
DROP TABLE "locations";
ALTER TABLE "new_locations" RENAME TO "locations";
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
