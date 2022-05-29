import type { File } from "./File";

export interface FilePath { id: number, is_dir: boolean, location_id: number, materialized_path: string, name: string, extension: string | null, file_id: number | null, parent_id: number | null, date_created: string, date_modified: string, date_indexed: string, file: File | null, }