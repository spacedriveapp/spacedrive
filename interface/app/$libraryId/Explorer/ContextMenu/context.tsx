import { createContext, PropsWithChildren, useContext } from 'react';
import {
	ExplorerItem,
	FilePath,
	NonIndexedPathItem,
	Object,
	useItemsAsEphemeralPaths,
	useItemsAsFilePaths,
	useItemsAsObjects
} from '@sd/client';
import { NonEmptyArray } from '~/util';

const ContextMenuContext = createContext<{
	selectedItems: NonEmptyArray<ExplorerItem>;
	selectedFilePaths: FilePath[];
	selectedObjects: Object[];
	selectedEphemeralPaths: NonIndexedPathItem[];
} | null>(null);

export const ContextMenuContextProvider = ({
	selectedItems,
	children
}: PropsWithChildren<{
	selectedItems: NonEmptyArray<ExplorerItem>;
}>) => {
	const selectedFilePaths = useItemsAsFilePaths(selectedItems);
	const selectedObjects = useItemsAsObjects(selectedItems);
	const selectedEphemeralPaths = useItemsAsEphemeralPaths(selectedItems);

	return (
		<ContextMenuContext.Provider
			value={{ selectedItems, selectedFilePaths, selectedObjects, selectedEphemeralPaths }}
		>
			{children}
		</ContextMenuContext.Provider>
	);
};

export const useContextMenuContext = () => {
	const context = useContext(ContextMenuContext);
	if (!context) throw new Error('ContextMenuContext.Provider not found');
	return context;
};
