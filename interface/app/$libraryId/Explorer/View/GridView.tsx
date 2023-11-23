import clsx from 'clsx';
import { memo } from 'react';
import { byteSize, getItemFilePath, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerStore } from '../store';
import { useExplorerDraggable } from '../useExplorerDraggable';
import { useExplorerDroppable } from '../useExplorerDroppable';
import Grid from './Grid';
import { RenamableItemText } from './RenamableItemText';
import { ViewItem } from './ViewItem';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
}

const GridViewItem = memo(({ data, selected, cut }: GridViewItemProps) => {
	const explorerStore = useExplorerStore();
	const explorerSettings = useExplorerContext().useSettingsSnapshot();

	const filePath = getItemFilePath(data);

	const isLocation = data.type === 'Location';
	const isEphemeral = data.type === 'NonIndexedPath';
	const isFolder = filePath?.is_dir;

	const showSize =
		explorerSettings.showBytesInGridView &&
		!isLocation &&
		!isFolder &&
		(!isEphemeral || !isFolder) &&
		(!explorerStore.isRenaming || !selected);

	const { isDroppable, navigateClassName, setDroppableRef } = useExplorerDroppable({
		data: { type: 'explorer-item', data: data },
		disabled: (!isFolder && !isLocation) || selected
	});

	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: data
	});

	return (
		<ViewItem data={data} className={clsx('h-full w-full', filePath?.hidden && 'opacity-50')}>
			<div ref={setDroppableRef}>
				<div
					className={clsx(
						'mb-1 aspect-square rounded-lg',
						(selected || isDroppable) && 'bg-app-selectedItem',
						navigateClassName
					)}
				>
					<FileThumb
						data={data}
						frame
						blackBars
						extension
						className={clsx('px-2 py-1', cut && 'opacity-60')}
						ref={setDraggableRef}
						childProps={{
							style,
							...attributes,
							...listeners
						}}
					/>
				</div>

				<div
					className="flex flex-col justify-center"
					ref={setDraggableRef}
					style={style}
					{...attributes}
					{...listeners}
				>
					<RenamableItemText
						item={data}
						style={{ maxHeight: 40 }}
						lines={2}
						highlight={isDroppable}
					/>
					{showSize && filePath?.size_in_bytes_bytes && (
						<span className="truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull">
							{`${byteSize(filePath.size_in_bytes_bytes)}`}
						</span>
					)}
				</div>
			</div>
		</ViewItem>
	);
});

export const GridView = () => {
	return (
		<Grid>
			{({ item, selected, cut }) => (
				<GridViewItem data={item} selected={selected} cut={cut} />
			)}
		</Grid>
	);
};
