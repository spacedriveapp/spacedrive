/*
  Warnings:

  - A unique constraint covering the columns `[client_id,mount_point,name]` on the table `volumes` will be added. If there are existing duplicate values, this will fail.

*/
-- CreateIndex
CREATE UNIQUE INDEX "volumes_client_id_mount_point_name_key" ON "volumes"("client_id", "mount_point", "name");
