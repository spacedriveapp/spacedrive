-- CreateTable
CREATE TABLE "storage_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "total_capacity" BIGINT NOT NULL DEFAULT 0,
    "available_capacity" BIGINT NOT NULL DEFAULT 0,
    "instance_pub_id" BLOB,
    CONSTRAINT "storage_statistics_instance_pub_id_fkey" FOREIGN KEY ("instance_pub_id") REFERENCES "instance" ("pub_id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "storage_statistics_pub_id_key" ON "storage_statistics"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "storage_statistics_instance_pub_id_key" ON "storage_statistics"("instance_pub_id");
