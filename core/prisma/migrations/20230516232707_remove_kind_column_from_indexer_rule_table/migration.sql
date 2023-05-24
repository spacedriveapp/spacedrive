/*
  Warnings:

  - You are about to drop the column `kind` on the `indexer_rule` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_indexer_rule" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "default" BOOLEAN NOT NULL DEFAULT false,
    "rules_per_kind" BLOB NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
DROP TABLE "indexer_rule";
ALTER TABLE "new_indexer_rule" RENAME TO "indexer_rule";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
