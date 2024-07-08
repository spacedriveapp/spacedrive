-- CreateTable
CREATE TABLE "object_kind_statistics" (
    "kind" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "total_bytes" BIGINT NOT NULL DEFAULT 0,
    "files_count" BIGINT NOT NULL DEFAULT 0
);
