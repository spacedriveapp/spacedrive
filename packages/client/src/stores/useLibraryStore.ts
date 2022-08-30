import { LibraryConfigWrapped } from '@sd/core';
import produce from 'immer';
import { useMemo } from 'react';
import create from 'zustand';
import { devtools, persist } from 'zustand/middleware';

import { useBridgeQuery } from '../index';
import { useExplorerStore } from './useExplorerStore';

type LibraryStore = {
	// the uuid of the currently active library
	currentLibraryUuid: string | null;
	// for full functionality this should be triggered along-side query invalidation
	switchLibrary: (uuid: string) => void;
	// a function
	init: (libraries: LibraryConfigWrapped[]) => Promise<void>;
};

export const useLibraryStore = create<LibraryStore>()(
	devtools(
		persist(
			(set) => ({
				currentLibraryUuid: null,
				switchLibrary: (uuid) => {
					set((state) =>
						produce(state, (draft) => {
							draft.currentLibraryUuid = uuid;
						})
					);
					// reset other stores
					useExplorerStore().reset();
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
			{ name: 'sd-library-store' }
		)
	)
);

// this must be used at least once in the app to correct the initial state
// is memorized and can be used safely in any component
export const useCurrentLibrary = () => {
	const { currentLibraryUuid, switchLibrary } = useLibraryStore();
	const { data: libraries } = useBridgeQuery(['library.get'], {
		onSuccess: (data) => {},
		onError: (err) => {}
	});

	// memorize library to avoid re-running find function
	const currentLibrary = useMemo(() => {
		const current = libraries?.find((l) => l.uuid === currentLibraryUuid);
		// switch to first library if none set
		if (Array.isArray(libraries) && !current && libraries[0]?.uuid) {
			switchLibrary(libraries[0]?.uuid);
		}
		return current;
	}, [libraries, currentLibraryUuid]);

	return { currentLibrary, libraries, currentLibraryUuid };
};
