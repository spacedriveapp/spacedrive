-- CreateTable
CREATE TABLE "owned_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "data" BLOB NOT NULL,
    "model" TEXT NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "owned_operation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "shared_operation" (
    "id" BLOB NOT NULL PRIMARY KEY,
    "timestamp" BIGINT NOT NULL,
    "model" TEXT NOT NULL,
    "record_id" BLOB NOT NULL,
    "kind" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "node_id" INTEGER NOT NULL,
    CONSTRAINT "shared_operation_node_id_fkey" FOREIGN KEY ("node_id") REFERENCES "node" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "name" TEXT,
    "extension" TEXT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL DEFAULT '0',
    "key_id" INTEGER,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbnail" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "note" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "object_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_object" ("cas_id", "date_created", "date_indexed", "date_modified", "extension", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "key_id", "kind", "name", "note", "size_in_bytes") SELECT "cas_id", "date_created", "date_indexed", "date_modified", "extension", "favorite", "has_thumbnail", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "integrity_checksum", "ipfs_id", "key_id", "kind", "name", "note", "size_in_bytes" FROM "object";
DROP TABLE "object";
ALTER TABLE "new_object" RENAME TO "object";
CREATE UNIQUE INDEX "object_cas_id_key" ON "object"("cas_id");
CREATE UNIQUE INDEX "object_integrity_checksum_key" ON "object"("integrity_checksum");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
