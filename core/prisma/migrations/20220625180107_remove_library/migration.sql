/*
  Warnings:

  - You are about to drop the `libraries` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the `library_statistics` table. If the table is not empty, all the data it contains will be lost.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "libraries";
PRAGMA foreign_keys=on;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "library_statistics";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "library_db_size" TEXT NOT NULL DEFAULT '0',
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "total_bytes_free" TEXT NOT NULL DEFAULT '0',
    "preview_media_bytes" TEXT NOT NULL DEFAULT '0'
);
