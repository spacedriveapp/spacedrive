-- CreateTable
CREATE TABLE "Volume" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "client_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "total_bytes_capacity" TEXT NOT NULL,
    "total_bytes_available" TEXT NOT NULL,
    "mount_point" TEXT NOT NULL,
    "disk_type" TEXT NOT NULL,
    "filesystem" TEXT NOT NULL
);
