import { LibraryConfigWrapped } from '@sd/core';
import produce from 'immer';
import { useMemo } from 'react';
import { useQueryClient } from 'react-query';
import create from 'zustand';
import { devtools, persist } from 'zustand/middleware';

import { useBridgeQuery } from '../bridge';
import { useExplorerStore } from './useExplorerStore';

type LibraryStore = {
	currentLibraryUuid: string | null;
	switchLibrary: (uuid: string) => void;
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
					const client = useQueryClient();
					client.invalidateQueries();
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

export const useCurrentLibrary = () => {
	const { currentLibraryUuid, switchLibrary } = useLibraryStore();
	const { data: libraries } = useBridgeQuery('NodeGetLibraries');

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
