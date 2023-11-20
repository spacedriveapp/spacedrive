-- CreateTable
CREATE TABLE "saved_search" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "filters" BLOB,
    "name" TEXT,
    "icon" TEXT,
    "description" TEXT,
    "order" INTEGER,
    "date_created" DATETIME,
    "date_modified" DATETIME
);

-- CreateIndex
CREATE UNIQUE INDEX "saved_search_pub_id_key" ON "saved_search"("pub_id");
