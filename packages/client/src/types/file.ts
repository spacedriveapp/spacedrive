import { FilePath, ObjectKind } from '@sd/core';

export interface ExplorerItem {
	id: number;
	name: string;
	is_dir: boolean;
	kind: ObjectKind;
	extension: string;
	size_in_bytes: number;
	created_at: string;
	updated_at: string;
	favorite?: boolean;

	// computed
	paths?: FilePath[];
}
