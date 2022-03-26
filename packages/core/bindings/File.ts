import type { EncryptionAlgorithm } from "./EncryptionAlgorithm";
import type { FilePath } from "./FilePath";
import type { FileType } from "./FileType";

export interface File { id: bigint, id_hash: string, name: string, extension: string | null, checksum: string | null, size_in_bytes: string, encryption: EncryptionAlgorithm, file_type: FileType, date_created: string, date_modified: string, date_indexed: string, ipfs_id: string | null, file_paths: Array<FilePath>, }