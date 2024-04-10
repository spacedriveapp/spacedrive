-- AlterTable
ALTER TABLE "job" ADD COLUMN "critical_error" TEXT;
ALTER TABLE "job" ADD COLUMN "non_critical_errors" BLOB;
