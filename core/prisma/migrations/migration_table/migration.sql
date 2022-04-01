-- CreateTable
CREATE TABLE "_migrations" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "checksum" TEXT NOT NULL,
    "steps_applied" INTEGER NOT NULL DEFAULT 0,
    "applied_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- CreateIndex
CREATE UNIQUE INDEX "_migrations_checksum_key" ON "_migrations"("checksum");