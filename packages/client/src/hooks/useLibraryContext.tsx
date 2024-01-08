import { createContext, PropsWithChildren, useContext } from 'react';

import { LibraryConfigWrapped } from '../core';
import { useBridgeSubscription } from '../rspc';
import { libraryStore, useLibraryStore } from '../stores/libraryStore';
import { ClientContext, useClientContext } from './useClientContext';

export interface LibraryContext {
	library: LibraryConfigWrapped;
	libraries: ClientContext['libraries'];
}

const LibraryContext = createContext<LibraryContext>(null!);

interface LibraryContextProviderProps extends PropsWithChildren {
	library: LibraryConfigWrapped;
}

export const LibraryContextProvider = ({ children, library }: LibraryContextProviderProps) => {
	const { libraries } = useClientContext();

	// We put this into context because each hook creates a new subscription which means we get duplicate events from the backend if we don't do this
	// TODO: This should probs be a library subscription - https://linear.app/spacedriveapp/issue/ENG-724/locationsonline-should-be-a-library-not-a-bridge-subscription
	useBridgeSubscription(['locations.online'], {
		onData: (d) => {
			libraryStore.onlineLocations = d;
		}
	});

	return (
		<LibraryContext.Provider value={{ library, libraries }}>{children}</LibraryContext.Provider>
	);
};

export const useLibraryContext = () => {
	const ctx = useContext(LibraryContext);

	if (ctx === undefined) throw new Error("'LibraryContextProvider' not mounted");

	return ctx;
};

export function useOnlineLocations() {
	const { onlineLocations } = useLibraryStore();
	return onlineLocations;
}
