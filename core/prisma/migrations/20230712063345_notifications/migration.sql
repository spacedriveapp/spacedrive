-- CreateTable
CREATE TABLE "notification" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "read" BOOLEAN NOT NULL DEFAULT false,
    "data" BLOB NOT NULL,
    "expires_at" DATETIME
);
