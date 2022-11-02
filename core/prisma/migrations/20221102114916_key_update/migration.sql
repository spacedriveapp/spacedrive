-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_key" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "uuid" TEXT NOT NULL,
    "name" TEXT,
    "default" BOOLEAN NOT NULL DEFAULT false,
    "date_created" DATETIME DEFAULT CURRENT_TIMESTAMP,
    "algorithm" BLOB NOT NULL,
    "hashing_algorithm" BLOB NOT NULL,
    "salt" BLOB NOT NULL,
    "content_salt" BLOB NOT NULL,
    "master_key" BLOB NOT NULL,
    "master_key_nonce" BLOB NOT NULL,
    "key_nonce" BLOB NOT NULL,
    "key" BLOB NOT NULL,
    "automount" BOOLEAN NOT NULL DEFAULT false
);
INSERT INTO "new_key" ("algorithm", "content_salt", "date_created", "default", "hashing_algorithm", "id", "key", "key_nonce", "master_key", "master_key_nonce", "name", "salt", "uuid") SELECT "algorithm", "content_salt", "date_created", "default", "hashing_algorithm", "id", "key", "key_nonce", "master_key", "master_key_nonce", "name", "salt", "uuid" FROM "key";
DROP TABLE "key";
ALTER TABLE "new_key" RENAME TO "key";
CREATE UNIQUE INDEX "key_uuid_key" ON "key"("uuid");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
