/*
  Warnings:

  - You are about to drop the column `has_thumbnail` on the `object` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_object" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "kind" INTEGER NOT NULL DEFAULT 0,
    "key_id" INTEGER,
    "hidden" BOOLEAN NOT NULL DEFAULT false,
    "favorite" BOOLEAN NOT NULL DEFAULT false,
    "important" BOOLEAN NOT NULL DEFAULT false,
    "has_thumbstrip" BOOLEAN NOT NULL DEFAULT false,
    "has_video_preview" BOOLEAN NOT NULL DEFAULT false,
    "ipfs_id" TEXT,
    "note" TEXT,
    "date_created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "date_accessed" DATETIME,
    CONSTRAINT "object_key_id_fkey" FOREIGN KEY ("key_id") REFERENCES "key" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
INSERT INTO "new_object" ("date_accessed", "date_created", "favorite", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "ipfs_id", "key_id", "kind", "note", "pub_id") SELECT "date_accessed", "date_created", "favorite", "has_thumbstrip", "has_video_preview", "hidden", "id", "important", "ipfs_id", "key_id", "kind", "note", "pub_id" FROM "object";
DROP TABLE "object";
ALTER TABLE "new_object" RENAME TO "object";
CREATE UNIQUE INDEX "object_pub_id_key" ON "object"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
