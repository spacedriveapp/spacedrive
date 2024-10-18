import { Plus } from '@phosphor-icons/react';
import { useMemo, type PropsWithChildren } from 'react';
import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { useLocale } from '~/hooks';
import { isNonEmpty } from '~/util';

import { useExplorerContext } from '../Context';
import { Conditional, type ConditionalGroupProps } from './ConditionalItem';
import { ContextMenuContextProvider } from './context';
import * as FilePathItems from './FilePath/Items';
import * as ObjectItems from './Object/Items';
import * as SharedItems from './SharedItems';

export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';
export * as SharedItems from './SharedItems';

const Items = ({ children }: PropsWithChildren) => {
	const { t } = useLocale();
	return (
		<>
			<Conditional items={[SharedItems.OpenOrDownload]} />
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

			{children}

			<ContextMenu.Separator />
			<SharedItems.Share />

			<SeparatedConditional items={[ObjectItems.AssignTag]} />

			<Conditional
				items={[
					FilePathItems.CopyAsPath,
					FilePathItems.Crypto,
					FilePathItems.Compress,
					ObjectItems.ConvertObject,
					FilePathItems.ParentFolderActions
					// FilePathItems.SecureDelete
				]}
			>
				{(items) => (
					<ContextMenu.SubMenu label={t('more_actions')} icon={Plus}>
						{items}
					</ContextMenu.SubMenu>
				)}
			</Conditional>

			<SeparatedConditional items={[FilePathItems.Delete]} />
		</>
	);
};

export default (props: PropsWithChildren<{ items?: ExplorerItem[]; custom?: boolean }>) => {
	const explorer = useExplorerContext();

	const selectedItems = useMemo(
		() => props.items || [...explorer.selectedItems],
		[explorer.selectedItems, props.items]
	);

	if (!isNonEmpty(selectedItems)) return null;

	return (
		<ContextMenuContextProvider selectedItems={selectedItems}>
			{props.custom ? <>{props.children}</> : <Items>{props.children}</Items>}
		</ContextMenuContextProvider>
	);
};

/**
 * A `Conditional` that inserts a `<ContextMenu.Separator />` above its items.
 */
export const SeparatedConditional = ({ items, children }: ConditionalGroupProps) => (
	<Conditional items={items}>
		{(c) => (
			<>
				<ContextMenu.Separator />
				{children ? children(c) : c}
			</>
		)}
	</Conditional>
);
