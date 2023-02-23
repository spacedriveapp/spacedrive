import { PropsWithChildren, createContext, useContext } from 'react';
import { LibraryConfigWrapped } from '../core';
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

	return (
		<LibraryContext.Provider value={{ library, libraries }}>{children}</LibraryContext.Provider>
	);
};

export const useLibraryContext = () => {
	const ctx = useContext(LibraryContext);

	if (ctx === undefined) throw new Error("'LibraryContextProvider' not mounted");

	return ctx;
};
