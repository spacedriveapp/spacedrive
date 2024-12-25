import { Transparent } from '@sd/assets/images';
import clsx from 'clsx';
import { memo, useEffect, useMemo } from 'react';
import {
	getItemFilePath,
	getItemObject,
	humanizeSize,
	libraryClient,
	Tag,
	useExplorerLayoutStore,
	useLibraryQuery,
	useSelector,
	type ExplorerItem
} from '@sd/client';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../../../Context';
import { ExplorerDraggable } from '../../../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../../../ExplorerDroppable';
import { FileThumb } from '../../../FilePath/Thumb';
import { useFrame } from '../../../FilePath/useFrame';
import { explorerStore } from '../../../store';
import { useExplorerDraggable } from '../../../useExplorerDraggable';
import { useExplorerItemData } from '../../../useExplorerItemData';
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

const InnerDroppable = memo(() => {
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
				<ItemFileThumb {...item} />
			</div>
			<ItemMetadata />
		</>
	);
});

const ItemFileThumb = memo((props: GridViewItemProps) => {
	const frame = useFrame();
	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: props.data
	});

	const isLabel = props.data.type === 'Label';

	return (
		<FileThumb
			data={props.data}
			frame={!isLabel}
			cover={isLabel}
			blackBars
			extension
			className={clsx(
				isLabel ? [frame.className, '!size-[90%] !rounded-md'] : 'px-2 py-1',
				props.cut && 'opacity-60'
			)}
			ref={setDraggableRef}
			childProps={useMemo(
				() => ({
					style,
					...attributes,
					...listeners
				}),
				[style, attributes, listeners]
			)}
		/>
	);
});

const ItemMetadata = memo(() => {
	const item = useGridViewItemContext();
	const { isDroppable } = useExplorerDroppableContext();
	const explorerLayout = useExplorerLayoutStore();
	const dragState = useSelector(explorerStore, (s) => s.drag);

	useEffect(() => {
		(async () => {
			if (dragState?.type === 'dragging' && dragState.items.length > 0) {
				const items = await Promise.all(
					dragState.items.map(async (item) => {
						const data = getItemFilePath(item);
						if (!data) return;

						const file_path =
							'path' in data
								? data.path
								: await libraryClient.query(['files.getPath', data.id]);

						return {
							type: 'explorer-item',
							file_path: file_path
						};
					})
				);

				// get image src from Transparent
				const image = Transparent.split('/@fs')[1];

				(window as any).startDrag({
					item: items.filter(Boolean).map((item) => item?.file_path),
					icon: image
				});
			}
		})();
	}, [dragState]);

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
});

const ItemTags = memo(() => {
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
});

const ItemSize = memo(() => {
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
});

const LabelItemCount = memo(({ data }: { data: Extract<ExplorerItem, { type: 'Label' }> }) => {
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

	if (count.data === undefined) return null;

	return (
		<div className="truncate rounded-md px-1.5 py-px text-center text-tiny text-ink-dull">
			{t('item_with_count', { count: count.data })}
		</div>
	);
});
