import { PropsWithChildren, createContext, useContext } from 'react';
import { ExplorerItem, FilePath, Object, useItemsAsFilePaths, useItemsAsObjects } from '@sd/client';
import { NonEmptyArray } from '~/util';

const ContextMenuContext = createContext<{
	selectedItems: NonEmptyArray<ExplorerItem>;
	selectedFilePaths: FilePath[];
	selectedObjects: Object[];
} | null>(null);

export const ContextMenuContextProvider = ({
	selectedItems,
	children
}: PropsWithChildren<{
	selectedItems: NonEmptyArray<ExplorerItem>;
}>) => {
	const selectedFilePaths = useItemsAsFilePaths(selectedItems);
	const selectedObjects = useItemsAsObjects(selectedItems);

	return (
		<ContextMenuContext.Provider value={{ selectedItems, selectedFilePaths, selectedObjects }}>
			{children}
		</ContextMenuContext.Provider>
	);
};

export const useContextMenuContext = () => {
	const context = useContext(ContextMenuContext);
	if (!context) throw new Error('ContextMenuContext.Provider not found');
	return context;
};
