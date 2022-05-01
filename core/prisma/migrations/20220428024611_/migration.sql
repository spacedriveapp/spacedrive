-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_volumes" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "is_system" BOOLEAN NOT NULL DEFAULT false,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_volumes" ("client_id", "date_modified", "disk_type", "filesystem", "id", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "client_id", "date_modified", "disk_type", "filesystem", "id", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "volumes";
DROP TABLE "volumes";
ALTER TABLE "new_volumes" RENAME TO "volumes";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
