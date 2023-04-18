/*
  Warnings:

  - You are about to drop the column `date_modified` on the `job` table. All the data in the column will be lost.
  - You are about to drop the column `seconds_elapsed` on the `job` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "data" BLOB,
    "metadata" BLOB,
    "parent_id" BLOB,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_started" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "date_completed" DATETIME DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "job_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_created", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_created", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
