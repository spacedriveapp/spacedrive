-- CreateTable
CREATE TABLE "album" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "is_hidden" BOOLEAN NOT NULL,
    "date_created" DATETIME NOT NULL,
    "date_modified" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "object_in_album" (
    "date_created" DATETIME NOT NULL,
    "album_id" INTEGER NOT NULL,
    "object_id" INTEGER NOT NULL,

    PRIMARY KEY ("album_id", "object_id"),
    CONSTRAINT "object_in_album_album_id_fkey" FOREIGN KEY ("album_id") REFERENCES "album" ("id") ON DELETE NO ACTION ON UPDATE CASCADE,
    CONSTRAINT "object_in_album_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE NO ACTION ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "album_pub_id_key" ON "album"("pub_id");
