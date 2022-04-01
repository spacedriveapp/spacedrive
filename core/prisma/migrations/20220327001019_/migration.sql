/*
  Warnings:

  - The primary key for the `jobs` table will be changed. If it partially fails, the table could be left without primary key constraint.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_jobs" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_jobs" ("action", "completed_task_count", "date_created", "date_modified", "id", "status", "task_count") SELECT "action", "completed_task_count", "date_created", "date_modified", "id", "status", "task_count" FROM "jobs";
DROP TABLE "jobs";
ALTER TABLE "new_jobs" RENAME TO "jobs";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
