import { useKey, useKeys } from 'rooks';
import type { ExplorerItem } from '@sd/client';
import { dialogManager } from '@sd/ui';
import DeleteDialog from '~/app/$libraryId/Explorer/FilePath/DeleteDialog';

import { useOperatingSystem } from './useOperatingSystem';

export const useKeyDeleteFile = (selectedItems: Set<ExplorerItem>, locationId?: number | null) => {
	const os = useOperatingSystem();

	const deleteHandler = () => {
		if (!locationId) return;

		const pathIds: number[] = [];

		for (const item of selectedItems) {
			if (item.type === 'Path') pathIds.push(item.item.id);
		}

		dialogManager.create((dp) => (
			<DeleteDialog {...dp} locationId={locationId} pathIds={pathIds} />
		));
	};

	useKeys(['Meta', 'Backspace'], (e) => {
		if (os !== 'macOS') return;
		e.preventDefault();
		deleteHandler();
	});

	useKey('Delete', (e) => {
		if (os === 'macOS') return;
		e.preventDefault();
		deleteHandler();
	});
};
