/*
  Warnings:

  - You are about to drop the column `logical_file_parent_id` on the `file_paths` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "materialized_path" TEXT NOT NULL,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "location_id" INTEGER NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "permissions" TEXT,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_file_paths" ("date_created", "date_indexed", "date_modified", "file_id", "id", "is_dir", "location_id", "materialized_path", "permissions") SELECT "date_created", "date_indexed", "date_modified", "file_id", "id", "is_dir", "location_id", "materialized_path", "permissions" FROM "file_paths";
DROP TABLE "file_paths";
ALTER TABLE "new_file_paths" RENAME TO "file_paths";
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_file_id_key" ON "file_paths"("location_id", "materialized_path", "file_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
