-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_Volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_Volume" ("client_id", "disk_type", "filesystem", "id", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "client_id", "disk_type", "filesystem", "id", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "Volume";
DROP TABLE "Volume";
ALTER TABLE "new_Volume" RENAME TO "Volume";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
