/*
  Warnings:

  - You are about to drop the column `path_checksum` on the `files` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "stem" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "quick_checksum" TEXT,
    "full_checksum" TEXT,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "ipfs_id" TEXT,
    "parent_id" INTEGER,
    CONSTRAINT "files_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "files_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_files" ("date_created", "date_indexed", "date_modified", "encryption", "extension", "full_checksum", "id", "ipfs_id", "is_dir", "location_id", "name", "parent_id", "quick_checksum", "size_in_bytes", "stem") SELECT "date_created", "date_indexed", "date_modified", "encryption", "extension", "full_checksum", "id", "ipfs_id", "is_dir", "location_id", "name", "parent_id", "quick_checksum", "size_in_bytes", "stem" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_location_id_stem_name_extension_key" ON "files"("location_id", "stem", "name", "extension");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
