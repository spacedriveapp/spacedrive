/*
  Warnings:

  - You are about to drop the column `total_byte_capacity` on the `libraries` table. All the data in the column will be lost.
  - You are about to drop the column `total_bytes_used` on the `libraries` table. All the data in the column will be lost.
  - You are about to drop the column `total_file_count` on the `libraries` table. All the data in the column will be lost.
  - You are about to drop the column `total_unique_bytes` on the `libraries` table. All the data in the column will be lost.

*/
-- CreateTable
CREATE TABLE "library_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "library_id" INTEGER NOT NULL,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_byte_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0'
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_libraries" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "remote_id" TEXT,
    "is_primary" BOOLEAN NOT NULL DEFAULT true,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT
);
INSERT INTO "new_libraries" ("date_created", "encryption", "id", "is_primary", "name", "remote_id", "timezone", "uuid") SELECT "date_created", "encryption", "id", "is_primary", "name", "remote_id", "timezone", "uuid" FROM "libraries";
DROP TABLE "libraries";
ALTER TABLE "new_libraries" RENAME TO "libraries";
CREATE UNIQUE INDEX "libraries_uuid_key" ON "libraries"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "library_statistics_library_id_key" ON "library_statistics"("library_id");
