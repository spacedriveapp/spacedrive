-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_file_path" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "is_dir" BOOLEAN,
    "cas_id" TEXT,
    "integrity_checksum" TEXT,
    "location_id" INTEGER,
    "materialized_path" TEXT,
    "name" TEXT COLLATE NOCASE,
    "extension" TEXT COLLATE NOCASE,
    "size_in_bytes" TEXT,
    "inode" BLOB,
    "device" BLOB,
    "object_id" INTEGER,
    "key_id" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME,
    "date_indexed" DATETIME,
    CONSTRAINT "file_path_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "location" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_path_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_file_path" ("cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes") SELECT "cas_id", "date_created", "date_indexed", "date_modified", "device", "extension", "id", "inode", "integrity_checksum", "is_dir", "key_id", "location_id", "materialized_path", "name", "object_id", "pub_id", "size_in_bytes" FROM "file_path";
DROP TABLE "file_path";
ALTER TABLE "new_file_path" RENAME TO "file_path";
CREATE UNIQUE INDEX "file_path_pub_id_key" ON "file_path"("pub_id");
CREATE INDEX "file_path_location_id_idx" ON "file_path"("location_id");
CREATE INDEX "file_path_location_id_materialized_path_idx" ON "file_path"("location_id", "materialized_path");
CREATE UNIQUE INDEX "file_path_location_id_materialized_path_name_extension_key" ON "file_path"("location_id", "materialized_path", "name", "extension");
CREATE UNIQUE INDEX "file_path_location_id_inode_device_key" ON "file_path"("location_id", "inode", "device");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
