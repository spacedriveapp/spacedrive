-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "keys" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_created", "date_indexed", "date_modified", "extension", "file_id", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "parent_id") SELECT "date_created", "date_indexed", "date_modified", "extension", "file_id", "id", "is_dir", "key_id", "location_id", "materialized_path", "name", "parent_id" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE INDEX "file_paths_location_id_idx" ON "file_paths"("location_id");
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
