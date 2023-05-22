import { useKey } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { dialogManager } from '@sd/ui';
import DeleteDialog from '../app/$libraryId/Explorer/File/DeleteDialog';

const useKeyDeleteFile = (selectedItem: ExplorerItem | null, location_id: number | null) => {
	return useKey('Delete', (e) => {
		e.preventDefault();

		if (!selectedItem || !location_id) return;

		dialogManager.create((dp) => (
			<DeleteDialog {...dp} location_id={location_id} path_id={selectedItem.item.id} />
		));
	});
};

export default useKeyDeleteFile;
