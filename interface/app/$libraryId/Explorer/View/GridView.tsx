import clsx from 'clsx';
import { memo } from 'react';
import { useMatch } from 'react-router';
import { byteSize, getItemFilePath, getItemLocation, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerStore } from '../store';
import Grid from './Grid';
import { RenamableItemText } from './RenamableItemText';
import { useExplorerDraggable } from './useExplorerDraggable';
import { useExplorerDroppable } from './useExplorerDroppable';
import { ViewItem } from './ViewItem';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
	isRenaming: boolean;
}

const GridViewItem = memo(({ data, selected, cut, isRenaming }: GridViewItemProps) => {
	const explorer = useExplorerContext();
	const { showBytesInGridView } = explorer.useSettingsSnapshot();

	const filePathData = getItemFilePath(data);
	const location = getItemLocation(data);
	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');
	const isFolder = filePathData?.is_dir;
	const hidden = filePathData?.hidden;

	const showSize =
		showBytesInGridView &&
		!location &&
		!isFolder &&
		(!isEphemeralLocation || !isFolder) &&
		(!isRenaming || !selected);

	const { isDroppable, navigateClassName, setDroppableRef } = useExplorerDroppable({
		data: { type: 'explorer-item', data: data },
		disabled: !isFolder || selected,
		allow:
			explorer.parent?.type === 'Location' ? 'Path' : explorer.parent ? 'Object' : undefined
	});

	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: data
	});

	return (
		<ViewItem data={data} className={clsx('h-full w-full', hidden && 'opacity-50')}>
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
					{showSize && filePathData?.size_in_bytes_bytes && (
						<span className="truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull">
							{`${byteSize(filePathData.size_in_bytes_bytes)}`}
						</span>
					)}
				</div>
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerStore = useExplorerStore();

	return (
		<Grid>
			{({ item, selected, cut }) => (
				<GridViewItem
					data={item}
					selected={selected}
					cut={cut}
					isRenaming={explorerStore.isRenaming}
				/>
			)}
		</Grid>
	);
};
