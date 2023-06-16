/*
  Warnings:

  - A unique constraint covering the columns `[pub_id]` on the table `indexer_rule` will be added. If there are existing duplicate values, this will fail.

*/
-- AlterTable
ALTER TABLE "indexer_rule" ADD COLUMN "pub_id" BLOB;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    "action" TEXT,
    "status" INTEGER NOT NULL DEFAULT 0,
    "errors_text" TEXT,
    "data" BLOB,
    "metadata" BLOB,
    "parent_id" BLOB,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_estimated_completion" DATETIME,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_started" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "date_completed" DATETIME,
    CONSTRAINT "job_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;

-- CreateIndex
CREATE UNIQUE INDEX "indexer_rule_pub_id_key" ON "indexer_rule"("pub_id");
