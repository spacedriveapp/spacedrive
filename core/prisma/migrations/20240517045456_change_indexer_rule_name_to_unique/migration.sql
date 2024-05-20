-- Update duplicate names to make them unique
UPDATE "indexer_rule"
SET "name" = "name" || '_' || "id"
WHERE "name" IN (
    SELECT "name"
    FROM "indexer_rule"
    GROUP BY "name"
    HAVING COUNT(*) > 1
);

-- CreateIndex
CREATE UNIQUE INDEX "indexer_rule_name_key" ON "indexer_rule"("name");
