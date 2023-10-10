import { useKey } from 'rooks';
import type { ExplorerItem } from '@sd/client';
import { dialogManager } from '@sd/ui';
import DeleteDialog from '~/app/$libraryId/Explorer/FilePath/DeleteDialog';

export const useKeyDeleteFile = (selectedItems: Set<ExplorerItem>, locationId?: number | null) => {
	return useKey('Delete', (e) => {
		e.preventDefault();

		if (!locationId) return;

		const pathIds: number[] = [];

		for (const item of selectedItems) {
			if (item.type === 'Path') pathIds.push(item.item.id);
		}

		dialogManager.create((dp) => (
			<DeleteDialog {...dp} locationId={locationId} pathIds={pathIds} />
		));
	});
};
