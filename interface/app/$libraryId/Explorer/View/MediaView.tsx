import clsx from 'clsx';
import { ArrowsOutSimple } from 'phosphor-react';
import { memo } from 'react';
import { ExplorerItem } from '@sd/client';
import { Button } from '@sd/ui';
import { ViewItem } from '.';
import { FileThumb } from '../FilePath/Thumb';
import { getExplorerStore, useExplorerStore } from '../store';
import GridList from './GridList';

interface MediaViewItemProps {
	data: ExplorerItem;
	selected: boolean;
	cut: boolean;
}

const MediaViewItem = memo(({ data, selected, cut }: MediaViewItemProps) => {
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
					data={data}
					cover={explorerStore.mediaAspectSquare}
					className={clsx(
						!explorerStore.mediaAspectSquare && 'px-1',
						cut && 'opacity-60'
					)}
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
	return (
		<GridList>
			{({ item, selected, cut }) => (
				<MediaViewItem data={item} selected={selected} cut={cut} />
			)}
		</GridList>
	);
};
