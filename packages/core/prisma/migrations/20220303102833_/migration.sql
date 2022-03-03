-- CreateTable
CREATE TABLE "libraries" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "remote_id" TEXT,
    "is_primary" BOOLEAN NOT NULL DEFAULT true,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_byte_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0',
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT
);

-- CreateTable
CREATE TABLE "clients" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL DEFAULT 0,
    "version" TEXT,
    "online" BOOLEAN DEFAULT true,
    "last_seen" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "locations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "is_removable" BOOLEAN NOT NULL DEFAULT true,
    "is_ejectable" BOOLEAN NOT NULL DEFAULT true,
    "is_root_filesystem" BOOLEAN NOT NULL DEFAULT true,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "path_integrity_hash" TEXT NOT NULL,
    "quick_integrity_hash" TEXT,
    "full_integrity_hash" TEXT,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "ipfs_id" TEXT,
    "parent_id" INTEGER,
    CONSTRAINT "files_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "files_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "encryption" INTEGER DEFAULT 0,
    "total_files" INTEGER DEFAULT 0,
    "redundancy_goal" INTEGER DEFAULT 1,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "tags_on_files" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "tag_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("tag_id", "file_id"),
    CONSTRAINT "tags_on_files_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "tags_on_files_tag_id_fkey" FOREIGN KEY ("tag_id") REFERENCES "tags" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "jobs" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "percentage_complete" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "jobs_client_id_fkey" FOREIGN KEY ("client_id") REFERENCES "clients" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "spaces" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "encryption" INTEGER DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "libraryId" INTEGER,
    CONSTRAINT "spaces_libraryId_fkey" FOREIGN KEY ("libraryId") REFERENCES "libraries" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "clients_uuid_key" ON "clients"("uuid");

-- CreateIndex
CREATE UNIQUE INDEX "files_path_integrity_hash_key" ON "files"("path_integrity_hash");
