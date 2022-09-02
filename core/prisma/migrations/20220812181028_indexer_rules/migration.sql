/*
  Warnings:

  - A unique constraint covering the columns `[local_path]` on the table `locations` will be added. If there are existing duplicate values, this will fail.

*/
-- CreateTable
CREATE TABLE "indexer_rules" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "parameters" BLOB NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "indexer_rules_in_location" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "location_id" INTEGER NOT NULL,
    "indexer_rule_id" INTEGER NOT NULL,

    PRIMARY KEY ("location_id", "indexer_rule_id"),
    CONSTRAINT "indexer_rules_in_location_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "indexer_rules_in_location_indexer_rule_id_fkey" FOREIGN KEY ("indexer_rule_id") REFERENCES "indexer_rules" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateIndex
CREATE UNIQUE INDEX "locations_local_path_key" ON "locations"("local_path");
