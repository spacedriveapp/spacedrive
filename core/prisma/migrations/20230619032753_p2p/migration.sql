-- RedefineTables
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_node" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pub_id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "platform" INTEGER NOT NULL,
    "date_created" DATETIME NOT NULL,
    "identity" BLOB,
    "node_peer_id" TEXT
);
INSERT INTO "new_node" ("date_created", "id", "name", "platform", "pub_id") SELECT "date_created", "id", "name", "platform", "pub_id" FROM "node";
DROP TABLE "node";
ALTER TABLE "new_node" RENAME TO "node";
CREATE UNIQUE INDEX "node_pub_id_key" ON "node"("pub_id");
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
