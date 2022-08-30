import create from 'zustand';

interface LibraryStore {
	currentLibraryUuid: string | null;
	switchLibrary: (id: string) => void;
}

export const useLibraryStore = create<LibraryStore>()((set) => ({
	currentLibraryUuid: null,
	switchLibrary: (uuid) => {
		set((state) => ({ currentLibraryUuid: uuid }));
	}
}));
