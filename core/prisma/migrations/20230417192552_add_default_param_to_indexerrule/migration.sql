-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_indexer_rule" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "default" BOOLEAN NOT NULL DEFAULT false,
    "parameters" BLOB NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_indexer_rule" ("date_created", "date_modified", "id", "kind", "name", "parameters") SELECT "date_created", "date_modified", "id", "kind", "name", "parameters" FROM "indexer_rule";
DROP TABLE "indexer_rule";
ALTER TABLE "new_indexer_rule" RENAME TO "indexer_rule";
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
