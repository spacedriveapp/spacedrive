-- CreateTable
CREATE TABLE "sync_events" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "timestamp" TEXT NOT NULL,
    "data" TEXT NOT NULL,
    CONSTRAINT "sync_events_client_id_fkey" FOREIGN KEY ("client_id") REFERENCES "clients" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "libraries" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "remote_id" TEXT,
    "is_primary" BOOLEAN NOT NULL DEFAULT true,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "timezone" TEXT
);

-- CreateTable
CREATE TABLE "library_statistics" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "date_captured" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "library_id" INTEGER NOT NULL,
    "total_file_count" INTEGER NOT NULL DEFAULT 0,
    "total_bytes_used" TEXT NOT NULL DEFAULT '0',
    "total_byte_capacity" TEXT NOT NULL DEFAULT '0',
    "total_unique_bytes" TEXT NOT NULL DEFAULT '0'
);

-- CreateTable
CREATE TABLE "clients" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
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
    "pub_id" TEXT NOT NULL,
    "client_id" INTEGER,
    "name" TEXT,
    "local_path" TEXT,
    "total_capacity" INTEGER,
    "available_capacity" INTEGER,
    "filesystem" TEXT,
    "disk_type" INTEGER,
    "is_removable" BOOLEAN NOT NULL DEFAULT true,
    "is_ejectable" BOOLEAN NOT NULL DEFAULT true,
    "is_root_filesystem" BOOLEAN NOT NULL DEFAULT true,
    "is_online" BOOLEAN NOT NULL DEFAULT true,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "files" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "cas_id" TEXT NOT NULL,
    "integrity_checksum" TEXT,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "size_in_bytes" TEXT NOT NULL,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbnail" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "comment" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "file_paths" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "is_dir" BOOLEAN NOT NULL DEFAULT false,
    "location_id" INTEGER NOT NULL,
    "materialized_path" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "extension" TEXT,
    "file_id" INTEGER,
    "parent_id" INTEGER,
    "encryption" INTEGER NOT NULL DEFAULT 0,
    "permissions" TEXT,
    "temp_cas_id" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_indexed" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "file_paths_location_id_fkey" FOREIGN KEY ("location_id") REFERENCES "locations" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "file_paths_parent_id_fkey" FOREIGN KEY ("parent_id") REFERENCES "file_paths" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "media_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pixel_width" INTEGER,
    "pixel_height" INTEGER,
    "longitude" REAL,
    "latitude" REAL,
    "capture_device" TEXT,
    "duration" INTEGER,
    CONSTRAINT "media_data_id_fkey" FOREIGN KEY ("id") REFERENCES "files" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "tags" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
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
CREATE TABLE "labels" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "label_on_files" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "label_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("label_id", "file_id"),
    CONSTRAINT "label_on_files_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "label_on_files_label_id_fkey" FOREIGN KEY ("label_id") REFERENCES "labels" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "jobs" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "client_id" INTEGER NOT NULL,
    "action" INTEGER NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "task_count" INTEGER NOT NULL DEFAULT 1,
    "completed_task_count" INTEGER NOT NULL DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "seconds_elapsed" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "jobs_client_id_fkey" FOREIGN KEY ("client_id") REFERENCES "clients" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "spaces" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "encryption" INTEGER DEFAULT 0,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "libraryId" INTEGER,
    CONSTRAINT "spaces_libraryId_fkey" FOREIGN KEY ("libraryId") REFERENCES "libraries" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "albums" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "files_in_albums" (
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "album_id" INTEGER NOT NULL,
    "file_id" INTEGER NOT NULL,

    PRIMARY KEY ("album_id", "file_id"),
    CONSTRAINT "files_in_albums_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION,
    CONSTRAINT "files_in_albums_album_id_fkey" FOREIGN KEY ("album_id") REFERENCES "albums" ("id") ON DELETE NO ACTION ON UPDATE NO ACTION
);

-- CreateTable
CREATE TABLE "comments" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_modified" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "file_id" INTEGER,
    CONSTRAINT "comments_file_id_fkey" FOREIGN KEY ("file_id") REFERENCES "files" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "libraries_pub_id_key" ON "libraries"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "library_statistics_library_id_key" ON "library_statistics"("library_id");

-- CreateIndex
CREATE UNIQUE INDEX "clients_pub_id_key" ON "clients"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "locations_pub_id_key" ON "locations"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "files_integrity_checksum_key" ON "files"("integrity_checksum");

-- CreateIndex
CREATE UNIQUE INDEX "file_paths_location_id_materialized_path_name_extension_key" ON "file_paths"("location_id", "materialized_path", "name", "extension");

-- CreateIndex
CREATE UNIQUE INDEX "tags_pub_id_key" ON "tags"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "labels_pub_id_key" ON "labels"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "spaces_pub_id_key" ON "spaces"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "albums_pub_id_key" ON "albums"("pub_id");

-- CreateIndex
CREATE UNIQUE INDEX "comments_pub_id_key" ON "comments"("pub_id");
