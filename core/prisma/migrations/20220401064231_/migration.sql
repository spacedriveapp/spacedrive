/*
  Warnings:

  - You are about to drop the column `type` on the `files` table. All the data in the column will be lost.

*/
-- AlterTable
ALTER TABLE "file_paths" ADD COLUMN "temp_checksum" TEXT;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
    "partial_checksum" TEXT NOT NULL,
    "checksum" TEXT,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "ipfs_id" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_files" ("checksum", "date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "partial_checksum", "size_in_bytes") SELECT "checksum", "date_created", "date_indexed", "date_modified", "encryption", "id", "ipfs_id", "partial_checksum", "size_in_bytes" FROM "files";
DROP TABLE "files";
ALTER TABLE "new_files" RENAME TO "files";
CREATE UNIQUE INDEX "files_checksum_key" ON "files"("checksum");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
