import { PropsWithChildren, createContext, useContext } from 'react';
import { ExplorerItem, FilePath, Object } from '@sd/client';
import { NonEmptyArray } from '~/util';
import { useItemsAsFilePaths } from './FilePath/utils';
import { useItemsAsObjects } from './Object/utils';

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
