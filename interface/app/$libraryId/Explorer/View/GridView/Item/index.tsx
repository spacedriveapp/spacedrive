import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { byteSize, getItemFilePath, useSelector, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../../../Context';
import { ExplorerDraggable } from '../../../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../../../ExplorerDroppable';
import { FileThumb } from '../../../FilePath/Thumb';
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
			<ViewItem data={props.data} className={clsx('h-full w-full', isHidden && 'opacity-50')}>
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
					'mb-1 aspect-square rounded-lg',
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
	const item = useGridViewItemContext();

	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: item.data
	});

	return (
		<FileThumb
			data={item.data}
			frame
			blackBars
			extension
			className={clsx('px-2 py-1', item.cut && 'opacity-60')}
			ref={setDraggableRef}
			frameClassName={clsx(item.data.type === "Label" && "!rounded-2xl")}
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

	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming && item.selected);

	return (
		<ExplorerDraggable draggable={{ data: item.data, disabled: isRenaming }}>
			<RenamableItemText
				item={item.data}
				style={{ maxHeight: 40, textAlign: 'center' }}
				lines={2}
				highlight={isDroppable}
				selected={item.selected}
			/>
			<ItemSize />
		</ExplorerDraggable>
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

	const showSize =
		showBytesInGridView &&
		filePath?.size_in_bytes_bytes &&
		!isLocation &&
		!isFolder &&
		(!isEphemeral || !isFolder) &&
		(!isRenaming || !item.selected);

	const bytes = useMemo(
		() => showSize && byteSize(filePath?.size_in_bytes_bytes),
		[filePath?.size_in_bytes_bytes, showSize]
	);

	if (!showSize) return null;

	return (
		<div className="truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull">
			{`${bytes}`}
		</div>
	);
};
