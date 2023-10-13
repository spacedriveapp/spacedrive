import clsx from 'clsx';
import { memo } from 'react';
import { useMatch } from 'react-router';
import { byteSize, getItemFilePath, getItemLocation, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import GridList from './GridList';
import { RenamableItemText } from './RenamableItemText';
import { ViewItem } from './ViewItem';

interface GridViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	isRenaming: boolean;
	cut: boolean;
}

const GridViewItem = memo(({ data, selected, cut, isRenaming }: GridViewItemProps) => {
	const explorer = useExplorerContext();
	const { showBytesInGridView, gridItemSize } = explorer.useSettingsSnapshot();

	const filePathData = getItemFilePath(data);
	const location = getItemLocation(data);
	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');
	const isFolder = 'is_dir' in data.item ? data.item.is_dir || data.type === 'Location' : false;

	const showSize =
		showBytesInGridView &&
		!isEphemeralLocation &&
		!isFolder &&
		!location &&
		(!isRenaming || (isRenaming && !selected));

	return (
		<ViewItem data={data} className="h-full w-full">
			<div
				className={clsx('mb-1 aspect-square rounded-lg', selected && 'bg-app-selectedItem')}
			>
				<FileThumb
					data={data}
					frame
					blackBars
					extension
					className={clsx('px-2 py-1', cut && 'opacity-60')}
				/>
			</div>

			<div className="flex flex-col justify-center">
				<RenamableItemText item={data} style={{ maxHeight: 40 }} lines={2} />
				{showSize && filePathData?.size_in_bytes_bytes && (
					<span
						className={clsx(
							'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-tiny text-ink-dull '
						)}
					>
						{`${byteSize(filePathData.size_in_bytes_bytes)}`}
					</span>
				)}
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerView = useExplorerViewContext();

	return (
		<GridList>
			{({ item, selected, cut }) => (
				<GridViewItem
					data={item}
					selected={selected}
					cut={cut}
					isRenaming={explorerView.isRenaming}
				/>
			)}
		</GridList>
	);
};
