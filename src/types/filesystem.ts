import { Encryption } from './library';
import { ImageMeta, VideoMeta } from './media';

export interface IFile {
  id?: number;
  meta_checksum: string;
  uri: string;
  is_dir: string;

  date_created: Date;
  date_modified: Date;
  date_indexed: Date;

  name: string;
  extension: string;
  size_in_bytes: string;

  library_id: string;
  ipfs_id: string;
  storage_device_id: string;
  capture_device_id: string;
  parent_id: string;
  tags?: ITag[];

  icon_b64?: string;
}

export interface IDirectory extends IFile {
  children?: IFile[];
  children_count: number;
}

export interface ITag {
  id: string;
}
