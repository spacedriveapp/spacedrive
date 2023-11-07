import { ArrowsOutSimple } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo } from 'react';
import { ExplorerItem, getItemFilePath } from '@sd/client';
import { Button } from '@sd/ui';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { getQuickPreviewStore } from '../QuickPreview/store';
import Grid from './Grid';
import { useExplorerDraggable } from './useExplorerDraggable';
import { ViewItem } from './ViewItem';

interface MediaViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
}

const MediaViewItem = memo(({ data, selected, cut }: MediaViewItemProps) => {
	const filePathData = getItemFilePath(data);

	const { mediaAspectSquare } = useExplorerContext().useSettingsSnapshot();

	const { setDraggableRef, attributes, listeners, style, isDragging } = useExplorerDraggable({
		data
	});

	return (
		<ViewItem
			data={data}
			className={clsx(
				'group relative h-full w-full border-2 hover:bg-app-selectedItem',
				selected ? 'border-accent bg-app-selectedItem' : 'border-transparent'
			)}
		>
			<FileThumb
				data={data}
				cover={mediaAspectSquare}
				extension
				className={clsx(
					!mediaAspectSquare && 'p-0.5',
					cut && 'opacity-60',
					filePathData?.hidden && 'opacity-50'
				)}
				ref={setDraggableRef}
				childProps={{
					style,
					...attributes,
					...listeners
				}}
			/>

			<Button
				variant="gray"
				size="icon"
				className={clsx(
					'absolute right-2 top-2 hidden rounded-full shadow',
					!isDragging && 'group-hover:block'
				)}
				onClick={() => (getQuickPreviewStore().open = true)}
			>
				<ArrowsOutSimple />
			</Button>
		</ViewItem>
	);
});

export const MediaView = () => {
	return (
		<Grid>
			{({ item, selected, cut }) => (
				<MediaViewItem data={item} selected={selected} cut={cut} />
			)}
		</Grid>
	);
};
