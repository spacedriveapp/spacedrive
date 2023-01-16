/*
  Warnings:

  - You are about to drop the `sync_event` table. If the table is not empty, all the data it contains will be lost.

*/
-- AlterTable
ALTER TABLE "file_path" ADD COLUMN "pending" BOOLEAN DEFAULT false;

-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "sync_event";
PRAGMA foreign_keys=on;
