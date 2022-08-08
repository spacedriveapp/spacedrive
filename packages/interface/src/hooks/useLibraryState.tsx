import { useBridgeQuery } from '@sd/client';
import { LibraryConfigWrapped } from '@sd/core';
import produce from 'immer';
import { useMemo } from 'react';
import create from 'zustand';
import { devtools, persist } from 'zustand/middleware';

interface LibraryState {
	currentLibraryUuid: string | null;
	switchLibrary: (uuid: string) => void;
	init: (libraries: LibraryConfigWrapped[]) => Promise<void>;
}

export const useLibraryState = create<LibraryState>()(
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
			{ name: 'sd-library-state' }
		)
	)
);

export const useCurrentLibrary = () => {
	const { currentLibraryUuid } = useLibraryState();
	const { data: libraries } = useBridgeQuery(['library.get']);

	// memorize library to avoid re-running find function
	const currentLibrary = useMemo(
		() => libraries?.find((l) => l.uuid === currentLibraryUuid),
		[libraries, currentLibraryUuid]
	);

	return { currentLibrary, libraries, currentLibraryUuid };
};
