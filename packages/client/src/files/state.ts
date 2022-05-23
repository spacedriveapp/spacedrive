import produce from 'immer';
import create from 'zustand';

export interface FileExplorerState {
	current_location_id: number | null;
	row_limit: number;
}

interface FileExplorerStore extends FileExplorerState {
	update_row_limit: (new_limit: number) => void;
}

export const useFileExplorerState = create<FileExplorerStore>((set, get) => ({
	current_location_id: null,
	row_limit: 10,
	update_row_limit: (new_limit: number) => {
		set((store) =>
			produce(store, (draft) => {
				draft.row_limit = new_limit;
			})
		);
	}
}));
