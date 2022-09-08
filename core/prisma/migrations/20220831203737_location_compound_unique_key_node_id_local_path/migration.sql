/*
  Warnings:

  - A unique constraint covering the columns `[node_id,local_path]` on the table `locations` will be added. If there are existing duplicate values, this will fail.

*/
-- DropIndex
DROP INDEX "locations_local_path_key";

-- CreateIndex
CREATE UNIQUE INDEX "locations_node_id_local_path_key" ON "locations"("node_id", "local_path");
