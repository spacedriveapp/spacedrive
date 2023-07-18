import clsx from 'clsx';
import { ArrowsOutSimple } from 'phosphor-react';
import { memo } from 'react';
import { ExplorerItem } from '@sd/client';
import { Button } from '@sd/ui';
import { ViewItem } from '.';
import FileThumb from '../FilePath/Thumb';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, useExplorerStore } from '../store';
import GridList from './GridList';

interface MediaViewItemProps {
	data: ExplorerItem;
	selected: boolean;
}

const MediaViewItem = memo(({ data, selected }: MediaViewItemProps) => {
	const explorerStore = useExplorerStore();

	return (
		<ViewItem
			data={data}
			className={clsx(
				'h-full w-full overflow-hidden border-2',
				selected ? 'border-accent' : 'border-transparent'
			)}
		>
			<div
				className={clsx(
					'group relative flex aspect-square items-center justify-center hover:bg-app-selectedItem',
					selected && 'bg-app-selectedItem'
				)}
			>
				<FileThumb
					size={0}
					data={data}
					cover={explorerStore.mediaAspectSquare}
					className="!rounded-none"
				/>

				<Button
					variant="gray"
					size="icon"
					className="absolute right-2 top-2 hidden rounded-full shadow group-hover:block"
					onClick={() => (getExplorerStore().quickViewObject = data)}
				>
					<ArrowsOutSimple />
				</Button>
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerView = useExplorerViewContext();

	return (
		<GridList>
			{(item) => {
				const isSelected =
					typeof explorerView.selected === 'object'
						? explorerView.selected.has(item.item.id)
						: explorerView.selected === item.item.id;

				return <MediaViewItem data={item} selected={isSelected} />;
			}}
		</GridList>
	);
};
