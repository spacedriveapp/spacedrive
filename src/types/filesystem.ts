import { Encryption } from './library';
import { ImageMeta, VideoMeta } from './media';

export interface Object {
  id?: number;
  checksum: string;
  type: ObjectType;

  uri: string;
  file_name: string;
  extension: string;
  size: number;
  mime: string;
  encryption?: Encryption;

  date_created: Date;
  date_modified: Date;
  date_indexed: Date;
  geolocation: string;

  directory_id: string;
  storage_device_id: string;
  capture_device_id: string;
  parent_object_id: string;
  user_id: string;

  extra_data: null | ImageMeta | VideoMeta;
  ipfs_id: string;
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
