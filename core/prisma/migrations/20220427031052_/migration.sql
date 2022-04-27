/*
  Warnings:

  - You are about to drop the `FileConflict` table. If the table is not empty, all the data it contains will be lost.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "FileConflict";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "file_conflicts" (
    "original_file_id" INTEGER NOT NULL,
    "detactched_file_id" INTEGER NOT NULL
);

-- CreateIndex
CREATE UNIQUE INDEX "file_conflicts_original_file_id_key" ON "file_conflicts"("original_file_id");

-- CreateIndex
CREATE UNIQUE INDEX "file_conflicts_detactched_file_id_key" ON "file_conflicts"("detactched_file_id");
