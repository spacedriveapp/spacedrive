-- AlterTable
ALTER TABLE "instance" ADD COLUMN "timestamp" BIGINT;

-- CreateTable
CREATE TABLE "relation_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "relation" TEXT NOT NULL,
    "item_id" BLOB NOT NULL,
    "group_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "instance_id" INTEGER NOT NULL,
    CONSTRAINT "relation_operation_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
