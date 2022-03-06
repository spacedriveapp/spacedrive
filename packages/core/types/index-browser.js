
Object.defineProperty(exports, "__esModule", { value: true });

const {
  Decimal
} = require('./runtime/index-browser')


const Prisma = {}

exports.Prisma = Prisma

/**
 * Prisma Client JS version: 3.10.0
 * Query Engine version: 73e60b76d394f8d37d8ebd1f8918c79029f0db86
 */
Prisma.prismaVersion = {
  client: "3.10.0",
  engine: "73e60b76d394f8d37d8ebd1f8918c79029f0db86"
}

Prisma.PrismaClientKnownRequestError = () => {
  throw new Error(`PrismaClientKnownRequestError is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)};
Prisma.PrismaClientUnknownRequestError = () => {
  throw new Error(`PrismaClientUnknownRequestError is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.PrismaClientRustPanicError = () => {
  throw new Error(`PrismaClientRustPanicError is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.PrismaClientInitializationError = () => {
  throw new Error(`PrismaClientInitializationError is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.PrismaClientValidationError = () => {
  throw new Error(`PrismaClientValidationError is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.Decimal = Decimal

/**
 * Re-export of sql-template-tag
 */
Prisma.sql = () => {
  throw new Error(`sqltag is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.empty = () => {
  throw new Error(`empty is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.join = () => {
  throw new Error(`join is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.raw = () => {
  throw new Error(`raw is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
)}
Prisma.validator = () => (val) => val

/**
 * Shorthand utilities for JSON filtering
 */
Prisma.DbNull = 'DbNull'
Prisma.JsonNull = 'JsonNull'
Prisma.AnyNull = 'AnyNull'

/**
 * Enums
 */
// Based on
// https://github.com/microsoft/TypeScript/issues/3192#issuecomment-261720275
function makeEnum(x) { return x; }

exports.Prisma.MigrationScalarFieldEnum = makeEnum({
  id: 'id',
  name: 'name',
  checksum: 'checksum',
  steps_applied: 'steps_applied',
  applied_at: 'applied_at'
});

exports.Prisma.LibraryScalarFieldEnum = makeEnum({
  id: 'id',
  uuid: 'uuid',
  name: 'name',
  remote_id: 'remote_id',
  is_primary: 'is_primary',
  encryption: 'encryption',
  date_created: 'date_created',
  timezone: 'timezone'
});

exports.Prisma.LibraryStatisticsScalarFieldEnum = makeEnum({
  id: 'id',
  date_captured: 'date_captured',
  library_id: 'library_id',
  total_file_count: 'total_file_count',
  total_bytes_used: 'total_bytes_used',
  total_byte_capacity: 'total_byte_capacity',
  total_unique_bytes: 'total_unique_bytes'
});

exports.Prisma.ClientScalarFieldEnum = makeEnum({
  id: 'id',
  uuid: 'uuid',
  name: 'name',
  platform: 'platform',
  version: 'version',
  online: 'online',
  last_seen: 'last_seen',
  timezone: 'timezone',
  date_created: 'date_created'
});

exports.Prisma.LocationScalarFieldEnum = makeEnum({
  id: 'id',
  name: 'name',
  path: 'path',
  total_capacity: 'total_capacity',
  available_capacity: 'available_capacity',
  is_removable: 'is_removable',
  is_ejectable: 'is_ejectable',
  is_root_filesystem: 'is_root_filesystem',
  is_online: 'is_online',
  date_created: 'date_created'
});

exports.Prisma.FileScalarFieldEnum = makeEnum({
  id: 'id',
  is_dir: 'is_dir',
  location_id: 'location_id',
  stem: 'stem',
  name: 'name',
  extension: 'extension',
  quick_checksum: 'quick_checksum',
  full_checksum: 'full_checksum',
  size_in_bytes: 'size_in_bytes',
  encryption: 'encryption',
  date_created: 'date_created',
  date_modified: 'date_modified',
  date_indexed: 'date_indexed',
  ipfs_id: 'ipfs_id',
  parent_id: 'parent_id'
});

exports.Prisma.TagScalarFieldEnum = makeEnum({
  id: 'id',
  name: 'name',
  encryption: 'encryption',
  total_files: 'total_files',
  redundancy_goal: 'redundancy_goal',
  date_created: 'date_created',
  date_modified: 'date_modified'
});

exports.Prisma.TagOnFileScalarFieldEnum = makeEnum({
  date_created: 'date_created',
  tag_id: 'tag_id',
  file_id: 'file_id'
});

exports.Prisma.JobScalarFieldEnum = makeEnum({
  id: 'id',
  client_id: 'client_id',
  action: 'action',
  status: 'status',
  percentage_complete: 'percentage_complete',
  task_count: 'task_count',
  completed_task_count: 'completed_task_count',
  date_created: 'date_created',
  date_modified: 'date_modified'
});

exports.Prisma.SpaceScalarFieldEnum = makeEnum({
  id: 'id',
  name: 'name',
  encryption: 'encryption',
  date_created: 'date_created',
  date_modified: 'date_modified',
  libraryId: 'libraryId'
});

exports.Prisma.SortOrder = makeEnum({
  asc: 'asc',
  desc: 'desc'
});


exports.Prisma.ModelName = makeEnum({
  Migration: 'Migration',
  Library: 'Library',
  LibraryStatistics: 'LibraryStatistics',
  Client: 'Client',
  Location: 'Location',
  File: 'File',
  Tag: 'Tag',
  TagOnFile: 'TagOnFile',
  Job: 'Job',
  Space: 'Space'
});

/**
 * Create the Client
 */
class PrismaClient {
  constructor() {
    throw new Error(
      `PrismaClient is unable to be run in the browser.
In case this error is unexpected for you, please report it in https://github.com/prisma/prisma/issues`,
    )
  }
}
exports.PrismaClient = PrismaClient

Object.assign(exports, Prisma)
