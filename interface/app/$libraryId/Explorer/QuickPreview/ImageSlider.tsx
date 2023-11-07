import { Grid, useGrid } from '@virtual-grid/react';
import clsx from 'clsx';
import { useCallback, useEffect, useRef, useState } from 'react';
import { ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';

export const ImageSlider = () => {
	const quickPreviewImagesRef = useRef<HTMLDivElement>(null);
	const explorer = useExplorerContext();
	const [activeIndex, setActiveIndex] = useState<null | number>(0);

	const grid = useGrid({
		scrollRef: quickPreviewImagesRef,
		count: explorer.items?.length ?? 0,
		totalCount: explorer.count,
		getItemData: useCallback((i: number) => explorer.items?.[i], [explorer.items]),
		onLoadMore: explorer.loadMore,
		columns: explorer.items?.length ?? 0,
		rowVirtualizer: { overscan: explorer.overscan ?? 5 },
		size: {
			width: 60,
			height: 60
		},
		gap: 10
	});

	const selectHandler = (i: number) => {
		const item = explorer.items?.[i];
		if (!item) return;
		explorer.resetSelectedItems([item]);
		setActiveIndex(i);
	};
	const activeItem = useCallback(
		(item: ExplorerItem): boolean => {
			const selectedItem = Array.from(explorer.selectedItems)[0];
			if (!selectedItem) return false;
			if (
				'item' in selectedItem &&
				'name' in selectedItem.item &&
				'item' in item &&
				'name' in item.item
			) {
				const getIndex = explorer.items?.findIndex((i) => {
					if ('id' in i.item && 'id' in selectedItem.item) {
						return i.item.id === selectedItem.item.id;
					}
					return false;
				});
				setActiveIndex(getIndex ?? 0);
				return selectedItem.item.name === item.item.name;
			}
			return false;
		},
		[explorer.selectedItems, explorer.items]
	);

	useEffect(() => {
		if (activeIndex === null) return;
		const gridItem = grid.getItem(activeIndex);
	}, [activeIndex, grid]);

	return (
		<div
			className={clsx(
				'relative mx-auto mb-2 flex w-full max-w-[700px] flex-row items-center justify-center',
				'rounded-md bg-white/5 '
			)}
		>
			<div
				ref={quickPreviewImagesRef}
				className="quick-preview-images-scroll w-f ull absolute bottom-0 mx-auto flex max-w-[700px] items-center justify-center overflow-y-hidden overflow-x-scroll py-3"
			>
				<Grid grid={grid}>
					{(i) => {
						const item = explorer.items?.[i];
						if (!item) return null;
						return (
							<div
								onClick={() => selectHandler(i)}
								key={i}
								className={clsx(
									'bg-white/5',
									'h-full w-full',
									'rounded-md',
									'border',
									activeItem(item)
										? 'border-2 border-accent'
										: 'border-1 border-white/5'
								)}
							>
								<FileThumb data={item} />
							</div>
						);
					}}
				</Grid>
			</div>
		</div>
	);
};
