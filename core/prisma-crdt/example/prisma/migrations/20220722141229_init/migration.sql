-- CreateTable
CREATE TABLE "shared_operations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BLOB NOT NULL,
    "data" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "shared_operations_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("local_id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "relation_operation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BLOB NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "model" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "relation_operation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("local_id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "RelationOperation" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "timestamp" BLOB NOT NULL,
    "relation" TEXT NOT NULL,
    "relation_item" BLOB NOT NULL,
    "relation_group" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "RelationOperation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("local_id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "nodes" (
    "local_id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "id" BLOB NOT NULL,
    "name" TEXT NOT NULL
);

-- CreateTable
CREATE TABLE "locations" (
    "local_id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "id" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    CONSTRAINT "locations_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "nodes" ("local_id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "file_paths" (
    "id" INTEGER NOT NULL,
    "location_id" INTEGER NOT NULL,
    "parent_id" INTEGER,
    "file_id" INTEGER,
    "name" TEXT NOT NULL,

    PRIMARY KEY ("location_id", "id"),
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("local_id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "file_paths_location_id_parent_id_fkey" FOREIGN KEY ("location_id", "parent_id") REFERENCES "file_paths" ("location_id", "id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("local_id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "files" (
    "local_id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" BLOB NOT NULL,
    "size_in_bytes" INTEGER NOT NULL DEFAULT 0
);

-- CreateTable
CREATE TABLE "tags" (
    "local_id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "id" BLOB NOT NULL,
    "name" TEXT NOT NULL
);

-- CreateTable
CREATE TABLE "tags_on_files" (
    "tag_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("tag_id", "file_id"),
    CONSTRAINT "tags_on_files_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("local_id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "tags_on_files_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tags" ("local_id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "nodes_id_key" ON "nodes"("id");

-- CreateIndex
CREATE UNIQUE INDEX "locations_id_key" ON "locations"("id");

-- CreateIndex
CREATE UNIQUE INDEX "files_cas_id_key" ON "files"("cas_id");

-- CreateIndex
CREATE UNIQUE INDEX "tags_id_key" ON "tags"("id");
