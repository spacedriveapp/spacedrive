import { Plus } from 'phosphor-react';
import { type ReactNode, useMemo } from 'react';
import { ContextMenu } from '@sd/ui';
import { isNonEmpty } from '~/util';
import { useExplorerContext } from '../Context';
import { Conditional, type ConditionalGroupProps } from './ConditionalItem';
import * as FilePathItems from './FilePath/Items';
import * as ObjectItems from './Object/Items';
import * as SharedItems from './SharedItems';
import { ContextMenuContextProvider } from './context';

export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';
export * as SharedItems from './SharedItems';

const Items = ({ children }: { children?: () => ReactNode }) => (
	<>
		<Conditional items={[FilePathItems.OpenOrDownload]} />
		<SharedItems.OpenQuickView />

		<SeparatedConditional items={[SharedItems.Details]} />

		<SeparatedConditional
			items={[
				SharedItems.RevealInNativeExplorer,
				SharedItems.Rename,
				FilePathItems.CutCopyItems,
				SharedItems.Deselect
			]}
		/>
		{children?.()}

		<ContextMenu.Separator />
		<SharedItems.Share />

		<SeparatedConditional items={[ObjectItems.AssignTag]} />

		<Conditional
			items={[
				FilePathItems.CopyAsPath,
				FilePathItems.Crypto,
				FilePathItems.Compress,
				ObjectItems.ConvertObject,
				FilePathItems.ParentFolderActions,
				FilePathItems.SecureDelete
			]}
		>
			{(items) => (
				<ContextMenu.SubMenu label="More actions..." icon={Plus}>
					{items}
				</ContextMenu.SubMenu>
			)}
		</Conditional>

		<SeparatedConditional items={[FilePathItems.Delete]} />
	</>
);

export default ({ children }: { children?: () => ReactNode }) => {
	const explorer = useExplorerContext();

	const selectedItems = useMemo(() => [...explorer.selectedItems], [explorer.selectedItems]);
	if (!isNonEmpty(selectedItems)) return null;

	return (
		<ContextMenuContextProvider selectedItems={selectedItems}>
			<Items>{children}</Items>
		</ContextMenuContextProvider>
	);
};

/**
 * A `Conditional` that inserts a `<ContextMenu.Separator />` above its items.
 */
const SeparatedConditional = ({ items, children }: ConditionalGroupProps) => (
	<Conditional items={items}>
		{(c) => (
			<>
				<ContextMenu.Separator />
				{children ? children(c) : c}
			</>
		)}
	</Conditional>
);
