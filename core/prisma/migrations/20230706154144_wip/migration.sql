/*
  Warnings:

  - You are about to drop the column `node_id` on the `job` table. All the data in the column will be lost.
  - You are about to drop the column `node_id` on the `volume` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT,
    "action" TEXT,
    "status" INTEGER,
    "errors_text" TEXT,
    "data" BLOB,
    "metadata" BLOB,
    "parent_id" BLOB,
    "task_count" INTEGER,
    "completed_task_count" INTEGER,
    "date_estimated_completion" DATETIME,
    "date_created" DATETIME,
    "date_started" DATETIME,
    "date_completed" DATETIME,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
CREATE TABLE "new_volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL DEFAULT '0',
    "total_bytes_available" TEXT NOT NULL DEFAULT '0',
    "disk_type" TEXT,
    "filesystem" TEXT,
    "is_system" BOOLEAN NOT NULL DEFAULT false,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_volume" ("date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity") SELECT "date_modified", "disk_type", "filesystem", "id", "is_system", "mount_point", "name", "total_bytes_available", "total_bytes_capacity" FROM "volume";
DROP TABLE "volume";
ALTER TABLE "new_volume" RENAME TO "volume";
CREATE UNIQUE INDEX "volume_mount_point_name_key" ON "volume"("mount_point", "name");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
