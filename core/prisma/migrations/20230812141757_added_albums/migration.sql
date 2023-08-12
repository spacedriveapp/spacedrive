-- CreateTable
CREATE TABLE "album" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT,
    "is_hidden" BOOLEAN,
    "date_created" DATETIME,
    "date_modified" DATETIME
);

-- CreateTable
CREATE TABLE "object_in_album" (
    "date_created" DATETIME,
    "album_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("album_id", "object_id"),
    CONSTRAINT "object_in_album_album_id_fkey" FOREIGN KEY ("album_id") REFERENCES "album" ("id") ON DELETE NO ACTION ON UPDATE CASCADE,
    CONSTRAINT "object_in_album_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "album_pub_id_key" ON "album"("pub_id");
