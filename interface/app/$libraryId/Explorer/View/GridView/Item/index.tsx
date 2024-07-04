import clsx from 'clsx';
import { memo, useMemo } from 'react';
import {
	getItemFilePath,
	getItemObject,
	humanizeSize,
	Tag,
	useExplorerLayoutStore,
	useLibraryQuery,
	useSelector,
	type ExplorerItem
} from '@sd/client';
import { useLocale } from '~/hooks';

import { useExplorerContext } from '../../../Context';
import { ExplorerDraggable } from '../../../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../../../ExplorerDroppable';
import { FileThumb } from '../../../FilePath/Thumb';
import { useFrame } from '../../../FilePath/useFrame';
import { explorerStore } from '../../../store';
import { useExplorerDraggable } from '../../../useExplorerDraggable';
import { RenamableItemText } from '../../RenamableItemText';
import { ViewItem } from '../../ViewItem';
import { GridViewItemContext, useGridViewItemContext } from './Context';

export interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
}

export const GridViewItem = memo((props: GridViewItemProps) => {
	const filePath = getItemFilePath(props.data);

	const isHidden = filePath?.hidden;
	const isFolder = filePath?.is_dir;
	const isLocation = props.data.type === 'Location';

	return (
		<GridViewItemContext.Provider value={props}>
			<ViewItem data={props.data} className={clsx('size-full', isHidden && 'opacity-50')}>
				<ExplorerDroppable
					droppable={{
						data: { type: 'explorer-item', data: props.data },
						disabled: (!isFolder && !isLocation) || props.selected
					}}
				>
					<InnerDroppable />
				</ExplorerDroppable>
			</ViewItem>
		</GridViewItemContext.Provider>
	);
});

const InnerDroppable = () => {
	const item = useGridViewItemContext();
	const { isDroppable } = useExplorerDroppableContext();
	return (
		<>
			<div
				className={clsx(
					'mb-1 flex aspect-square items-center justify-center rounded-lg',
					(item.selected || isDroppable) && 'bg-app-selectedItem'
				)}
			>
				<ItemFileThumb />
			</div>
			<ItemMetadata />
		</>
	);
};

const ItemFileThumb = () => {
	const frame = useFrame();

	const item = useGridViewItemContext();
	const isLabel = item.data.type === 'Label';

	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: item.data
	});

	return (
		<FileThumb
			data={item.data}
			frame={!isLabel}
			cover={isLabel}
			blackBars
			extension
			className={clsx(
				isLabel ? [frame.className, '!size-[90%] !rounded-md'] : 'px-2 py-1',
				item.cut && 'opacity-60'
			)}
			ref={setDraggableRef}
			childProps={{
				style,
				...attributes,
				...listeners
			}}
		/>
	);
};

const ItemMetadata = () => {
	const item = useGridViewItemContext();
	const { isDroppable } = useExplorerDroppableContext();
	const explorerLayout = useExplorerLayoutStore();

	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming && item.selected);

	return (
		<ExplorerDraggable draggable={{ data: item.data, disabled: isRenaming }}>
			<RenamableItemText
				item={item.data}
				style={{ textAlign: 'center' }}
				lines={2}
				editLines={3}
				highlight={isDroppable}
				selected={item.selected}
			/>
			<ItemSize />
			{explorerLayout.showTags && <ItemTags />}
			{item.data.type === 'Label' && <LabelItemCount data={item.data} />}
		</ExplorerDraggable>
	);
};

const ItemTags = () => {
	const item = useGridViewItemContext();
	const object = getItemObject(item.data);
	const filePath = getItemFilePath(item.data);
	const data = object || filePath;
	const tags = data && 'tags' in data ? data.tags : [];
	return (
		<div
			className="relative mt-1 flex w-full flex-row items-center justify-center"
			style={{
				left: tags.length * 1
			}}
		>
			{tags?.slice(0, 3).map((tag: { tag: Tag }, i: number) => (
				<div
					key={tag.tag.id}
					className="relative size-2.5 rounded-full border border-app"
					style={{
						backgroundColor: tag.tag.color!,
						right: i * 4
					}}
				/>
			))}
		</div>
	);
};

const ItemSize = () => {
	const item = useGridViewItemContext();
	const { showBytesInGridView } = useExplorerContext().useSettingsSnapshot();
	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming);

	const filePath = getItemFilePath(item.data);

	const isLocation = item.data.type === 'Location';
	const isEphemeral = item.data.type === 'NonIndexedPath';
	const isFolder = filePath?.is_dir;
	const { t } = useLocale();

	const showSize =
		showBytesInGridView &&
		filePath?.size_in_bytes_bytes &&
		!isLocation &&
		!isFolder &&
		(!isEphemeral || !isFolder) &&
		(!isRenaming || !item.selected);

	const bytes = useMemo(
		() =>
			showSize &&
			`${humanizeSize(filePath?.size_in_bytes_bytes).value} ${t(`size_${humanizeSize(filePath?.size_in_bytes_bytes).unit.toLowerCase()}`)}`,
		[filePath?.size_in_bytes_bytes, showSize, t]
	);

	if (!showSize) return null;

	return (
		<div className="truncate rounded-md px-1.5 py-px text-center text-tiny text-ink-dull">
			{`${bytes}`}
		</div>
	);
};

function LabelItemCount({ data }: { data: Extract<ExplorerItem, { type: 'Label' }> }) {
	const { t } = useLocale();

	const count = useLibraryQuery([
		'search.objectsCount',
		{
			filters: [
				{
					object: { labels: { in: [data.item.id] } }
				}
			]
		}
	]);

	if (count.data === undefined) return;

	return (
		<div className="truncate rounded-md px-1.5 py-px text-center text-tiny text-ink-dull">
			{t('item_with_count', { count: count.data })}
		</div>
	);
}
