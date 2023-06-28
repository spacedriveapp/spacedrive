import { PropsWithChildren, createContext, useContext, useState } from 'react';
import { LibraryConfigWrapped } from '../core';
import { useBridgeSubscription } from '../rspc';
import { ClientContext, useClientContext } from './useClientContext';

export interface LibraryContext {
	library: LibraryConfigWrapped;
	libraries: ClientContext['libraries'];
	onlineLocations: number[][] | null;
}

const LibraryContext = createContext<LibraryContext>(null!);

interface LibraryContextProviderProps extends PropsWithChildren {
	library: LibraryConfigWrapped;
}

export const LibraryContextProvider = ({ children, library }: LibraryContextProviderProps) => {
	const { libraries } = useClientContext();
	const [onlineLocations, setOnlineLocations] = useState<number[][] | null>(null);

	// We put this into context because each hook creates a new subscription which means we get duplicate events from the backend if we don't do this
	// TODO: This should probs be a library subscription - https://linear.app/spacedriveapp/issue/ENG-724/locationsonline-should-be-a-library-not-a-bridge-subscription
	useBridgeSubscription(['locations.online'], {
		onData: (d) => setOnlineLocations(d)
	});

	return (
		<LibraryContext.Provider value={{ library, libraries, onlineLocations }}>
			{children}
		</LibraryContext.Provider>
	);
};

export const useLibraryContext = () => {
	const ctx = useContext(LibraryContext);

	if (ctx === undefined) throw new Error("'LibraryContextProvider' not mounted");

	return ctx;
};

export function useOnlineLocations() {
	const ctx = useLibraryContext();
	return ctx?.onlineLocations || [];
}
