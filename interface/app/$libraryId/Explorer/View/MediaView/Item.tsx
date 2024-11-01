import { ArrowsOutSimple } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo } from 'react';
import { ExplorerItem, getItemFilePath } from '@sd/client';
import { Button } from '@sd/ui';

import { FileThumb } from '../../FilePath/Thumb';
import { getQuickPreviewStore } from '../../QuickPreview/store';
import { useExplorerDraggable } from '../../useExplorerDraggable';
import { ViewItem } from '../ViewItem';

interface Props {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
	cover: boolean;
}

export const MediaViewItem = memo(({ data, selected, cut, cover }: Props) => {
	return (
		<ViewItem
			data={data}
			className={clsx(
				'group relative size-full border-2 hover:bg-app-selectedItem',
				selected ? 'border-accent bg-app-selectedItem' : 'border-transparent'
			)}
		>
			<ItemFileThumb data={data} cut={cut} cover={cover} />

			<Button
				variant="gray"
				size="icon"
				className="absolute right-1 top-1 hidden !rounded shadow group-hover:block"
				onClick={() => (getQuickPreviewStore().open = true)}
			>
				<ArrowsOutSimple />
			</Button>
		</ViewItem>
	);
});

const ItemFileThumb = (props: Pick<Props, 'data' | 'cut' | 'cover'>) => {
	const filePath = getItemFilePath(props.data);

	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable({
		data: props.data
	});

	return (
		<FileThumb
			data={props.data}
			cover={props.cover}
			extension
			className={clsx(
				!props.cover && 'p-0.5',
				props.cut && 'opacity-60',
				filePath?.hidden && 'opacity-50'
			)}
			ref={setDraggableRef}
			childClassName={(type) => clsx(type === 'icon' && 'size-2/4')}
			childProps={{
				style,
				...attributes,
				...listeners
			}}
		/>
	);
};
