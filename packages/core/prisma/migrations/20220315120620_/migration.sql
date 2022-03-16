/*
  Warnings:

  - You are about to drop the column `percentage_complete` on the `jobs` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_jobs" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "jobs_client_id_fkey" FOREIGN KEY ("client_id") REFERENCES "clients" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);
INSERT INTO "new_jobs" ("action", "client_id", "completed_task_count", "date_created", "date_modified", "id", "status", "task_count") SELECT "action", "client_id", "completed_task_count", "date_created", "date_modified", "id", "status", "task_count" FROM "jobs";
DROP TABLE "jobs";
ALTER TABLE "new_jobs" RENAME TO "jobs";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
