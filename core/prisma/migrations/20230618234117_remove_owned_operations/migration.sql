/*
  Warnings:

  - You are about to drop the `owned_operation` table. If the table is not empty, all the data it contains will be lost.
  - You are about to drop the column `date_created` on the `indexer_rule_in_location` table. All the data in the column will be lost.
  - Made the column `pub_id` on table `indexer_rule` required. This step will fail if there are existing NULL values in that column.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "owned_operation";
PRAGMA foreign_keys=on;

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_indexer_rule" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "default" BOOLEAN,
    "rules_per_kind" BLOB,
    "date_created" DATETIME,
    "date_modified" DATETIME
);
INSERT INTO "new_indexer_rule" ("date_created", "date_modified", "default", "id", "name", "pub_id", "rules_per_kind") SELECT "date_created", "date_modified", "default", "id", "name", "pub_id", "rules_per_kind" FROM "indexer_rule";
DROP TABLE "indexer_rule";
ALTER TABLE "new_indexer_rule" RENAME TO "indexer_rule";
CREATE UNIQUE INDEX "indexer_rule_pub_id_key" ON "indexer_rule"("pub_id");
CREATE TABLE "new_job" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "name" TEXT,
    "node_id" INTEGER,
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
    CONSTRAINT "job_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "job_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "job" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_job" ("action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count") SELECT "action", "completed_task_count", "data", "date_completed", "date_created", "date_estimated_completion", "date_started", "errors_text", "id", "metadata", "name", "node_id", "parent_id", "status", "task_count" FROM "job";
DROP TABLE "job";
ALTER TABLE "new_job" RENAME TO "job";
CREATE TABLE "new_indexer_rule_in_location" (
    "location_id" INTEGER NOT NULL,
    "indexer_rule_id" INTEGER NOT NULL,

    PRIMARY KEY ("location_id", "indexer_rule_id"),
    CONSTRAINT "indexer_rule_in_location_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "indexer_rule_in_location_indexer_rule_id_fkey" FOREIGN KEY ("indexer_rule_id") REFERENCES "indexer_rule" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);
INSERT INTO "new_indexer_rule_in_location" ("indexer_rule_id", "location_id") SELECT "indexer_rule_id", "location_id" FROM "indexer_rule_in_location";
DROP TABLE "indexer_rule_in_location";
ALTER TABLE "new_indexer_rule_in_location" RENAME TO "indexer_rule_in_location";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
