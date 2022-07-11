import produce from 'immer';
import { debounce } from 'lodash';
import create from 'zustand';

import { libraryCommand } from '../bridge';

export type UpdateNoteFN = (vars: { id: number; note: string }) => void;

interface InspectorStore {
	notes: Record<number, string>;
	setNote: (file_id: number, note: string) => void;
	unCacheNote: (file_id: number) => void;
}

export const useInspectorStore = create<InspectorStore>((set) => ({
	notes: {},
	// set the note locally
	setNote: (file_id, note) => {
		set((state) => {
			const change = produce(state, (draft) => {
				draft.notes[file_id] = note;
			});
			updateNote(file_id, note);
			return change;
		});
	},
	// remove local note once confirmed saved server-side
	unCacheNote: (file_id) => {
		set((state) =>
			produce(state, (draft) => {
				delete draft.notes[file_id];
			})
		);
	}
}));

// direct command call to update note
export const updateNote = debounce(async (file_id: number, note: string) => {
	return await libraryCommand('FileSetNote', {
		id: file_id,
		note
	});
}, 500);
