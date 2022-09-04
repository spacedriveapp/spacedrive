import AsyncStorage from '@react-native-async-storage/async-storage';
import produce from 'immer';
import { useMemo } from 'react';
import create from 'zustand';
import { persist } from 'zustand/middleware';
import { useBridgeQuery } from '~/hooks/rspc';
import { LibraryConfigWrapped } from '~/types/bindings';

interface LibraryStore {
	_hasHydrated: boolean;
	setHasHydrated: (hasHydrated: boolean) => void;
	currentLibraryUuid: string | null;
	switchLibrary: (id: string) => void;
	init: (libraries: LibraryConfigWrapped[]) => Promise<void>;
}

export const useLibraryStore = create<LibraryStore>()(
	persist(
		(set) => ({
			_hasHydrated: false,
			setHasHydrated: (state) => {
				set({ _hasHydrated: state });
			},
			currentLibraryUuid: null,
			switchLibrary: (uuid) => {
				set((state) =>
					produce(state, (draft) => {
						draft.currentLibraryUuid = uuid;
					})
				);
				// reset other stores
			},
			init: async (libraries) => {
				set((state) =>
					produce(state, (draft) => {
						// use first library default if none set
						if (!state.currentLibraryUuid) {
							draft.currentLibraryUuid = libraries[0].uuid;
						}
					})
				);
			}
		}),
		{
			name: 'sd-library-store',
			getStorage: () => AsyncStorage,
			// Since storage is async, app needs to stay in loading state until hydrated!
			onRehydrateStorage: () => (state) => {
				state.setHasHydrated(true);
			}
		}
	)
);

// this must be used at least once in the app to correct the initial state
// is memorized and can be used safely in any component
export const useCurrentLibrary = () => {
	const { currentLibraryUuid, switchLibrary } = useLibraryStore();
	const { data: libraries } = useBridgeQuery(['library.get']);

	// memorize library to avoid re-running find function
	const currentLibrary = useMemo(() => {
		const current = libraries?.find((l) => l.uuid === currentLibraryUuid);
		// switch to first library if none set
		if (Array.isArray(libraries) && !current && libraries[0]?.uuid) {
			switchLibrary(libraries[0]?.uuid);
		}
		return current;
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [libraries, currentLibraryUuid]);

	return { currentLibrary, libraries, currentLibraryUuid };
};
