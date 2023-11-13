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
		size: 60,
		horizontal: true,
		onLoadMore: explorer.loadMore,
		getItemData: useCallback((i: number) => explorer.items?.[i], [explorer.items]),
		gap: 10,
		padding: 12,
		overscan: explorer.overscan ?? 5
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
				const getIndex = explorer.items?.findIndex((i, idx) => {
					if ('id' in i.item && 'id' in selectedItem.item) {
						return i.item.id === selectedItem.item.id;
					}
					return false;
				});
				if (getIndex === undefined) return false;
				setActiveIndex(getIndex);
				return selectedItem.item.name === item.item.name;
			}
			return false;
		},
		[explorer.selectedItems, explorer.items]
	);

	useEffect(() => {
		if (activeIndex === null) return;

		const container = quickPreviewImagesRef.current;
		const gridItem = grid.getItem(activeIndex);
		if (!container || !gridItem) return;

		// Calculate the scroll position required to bring the active item into view
		const containerWidth = container.clientWidth;
		const itemLeft = gridItem.rect.left;
		const itemRight = gridItem.rect.right;
		const containerScrollLeft = container.scrollLeft;

		if (itemLeft < containerScrollLeft) {
			// Active item is to the left of the visible area
			container.scrollTo({
				left: itemLeft,
				behavior: 'smooth'
			});
		} else if (itemRight > containerScrollLeft + containerWidth) {
			// Active item is to the right of the visible area
			container.scrollTo({
				left: itemRight - containerWidth + 20,
				behavior: 'smooth'
			});
		}
	}, [activeIndex, grid]);

	return (
		<div
			className={clsx(
				'relative mx-auto mb-4 flex w-full max-w-[700px] flex-row items-center justify-center',
				'rounded-md '
			)}
		>
			<div
				ref={quickPreviewImagesRef}
				className="quick-preview-images-scroll w-full overflow-x-auto overflow-y-hidden rounded-md bg-white/20 backdrop-blur-md"
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
