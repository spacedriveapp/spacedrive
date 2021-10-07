import { Encryption } from './library';
import { ImageMeta, VideoMeta } from './media';

export interface FileData {
  id?: number;
  meta_checksum: string;
  uri: string;

  date_created: Date;
  date_modified: Date;
  date_indexed: Date;

  name: string;
  extension: string;
  size_in_bytes: string;

  library_id: string;
  directory_id: string;
  ipfs_id: string;
  storage_device_id: string;
  capture_device_id: string;
  parent_file_id: string;
}

export interface Directory {
  id: string;
  name: string;

  calculated_size: string;
  calculated_object_count: number;
  storage_device_id: string;
  parent_directory_id: string;
  user_id: string;

  date_created: Date;
  date_modified: Date;
  date_indexed: Date;
}

export interface Tag {
  id: string;
}

export interface TagObject {
  object_id: string;
  tag_id: string;
}

export enum ObjectType {
  FILE,
  LINK
}
