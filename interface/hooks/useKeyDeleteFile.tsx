import { useKey, useKeys } from 'rooks';
import type { ExplorerItem } from '@sd/client';
import { dialogManager } from '@sd/ui';
import DeleteDialog from '~/app/$libraryId/Explorer/FilePath/DeleteDialog';

import { useOperatingSystem } from './useOperatingSystem';

export const useKeyDeleteFile = (selectedItems: Set<ExplorerItem>, locationId?: number | null) => {
	const os = useOperatingSystem();

	const deleteHandler = (e: KeyboardEvent) => {
		e.preventDefault();
		if (!locationId || selectedItems.size === 0) return;

		const pathIds: number[] = [];
		let dirCount = 0;
		let fileCount = 0;

		for (const item of selectedItems) {
			if (item.type === 'Path') {
				pathIds.push(item.item.id);

				dirCount += item.item.is_dir ? 1 : 0;
				fileCount += item.item.is_dir ? 0 : 1;
			}
		}

		dialogManager.create((dp) => (
			<DeleteDialog
				{...dp}
				locationId={locationId}
				pathIds={pathIds}
				dirCount={dirCount}
				fileCount={fileCount}
			/>
		));
	};

	useKeys(['Meta', 'Backspace'], (e) => {
		if (os !== 'macOS') return;
		deleteHandler(e);
	});

	useKey('Delete', (e) => {
		if (os === 'macOS') return;
		deleteHandler(e);
	});
};
