import type { EncryptionAlgorithm } from './EncryptionAlgorithm';
import type { FileKind } from './FileKind';
import type { FilePath } from './FilePath';

export interface File {
  id: number;
  cas_id: string;
  integrity_checksum: string | null;
  size_in_bytes: string;
  kind: FileKind;
  hidden: boolean;
  favorite: boolean;
  important: boolean;
  has_thumbnail: boolean;
  has_thumbstrip: boolean;
  has_video_preview: boolean;
  encryption: EncryptionAlgorithm;
  ipfs_id: string | null;
  comment: string | null;
  date_created: string;
  date_modified: string;
  date_indexed: string;
  paths: Array<FilePath>;
}
