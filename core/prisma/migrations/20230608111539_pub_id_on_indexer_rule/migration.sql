/*
  Warnings:

  - A unique constraint covering the columns `[pub_id]` on the table `indexer_rule` will be added. If there are existing duplicate values, this will fail.

*/
-- AlterTable
ALTER TABLE "indexer_rule" ADD COLUMN "pub_id" BLOB;

-- CreateIndex
CREATE UNIQUE INDEX "indexer_rule_pub_id_key" ON "indexer_rule"("pub_id");
