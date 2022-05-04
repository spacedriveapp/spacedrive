/*
  Warnings:

  - You are about to drop the `Volume` table. If the table is not empty, all the data it contains will be lost.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "Volume";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "volumes" (
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
