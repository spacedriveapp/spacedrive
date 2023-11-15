import { Grid, useGrid } from '@virtual-grid/react';
import clsx from 'clsx';
import { useCallback, useEffect, useRef } from 'react';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { uniqueId } from '../util';

export const ImageSlider = () => {
	const explorer = useExplorerContext();
	const quickPreviewImagesRef = useRef<HTMLDivElement>(null);
	const activeIndex = useRef<number | null>(null);

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

	const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
		const container = quickPreviewImagesRef.current;
		if (!container) return;

		//dont scroll if at the end
		if (
			(e.deltaX < 0 && container.scrollLeft === 0) ||
			(e.deltaX > 0 && container.scrollLeft === container.scrollWidth - container.clientWidth)
		)
			return;

		const delta = e.deltaY || e.deltaX;
		container.scrollLeft += delta;

		// Prevent the default wheel behavior to avoid unwanted scrolling
		e.preventDefault();
	};

	useEffect(() => {
		const container = quickPreviewImagesRef.current;
		if (!container) return;

		if (explorer.selectedItems.size !== 1) return;

		const [item] = [...explorer.selectedItems];
		if (!item) return;

		const index = explorer.items?.findIndex((_item) => uniqueId(_item) === uniqueId(item));
		if (index === undefined || index === -1) return;

		if (activeIndex.current === index) {
			container.scrollTo({
				left: grid.getItemRect(activeIndex.current).left - container.clientWidth / 2
			});
			return;
		}

		if (activeIndex.current === null) activeIndex.current = index;

		const { left: rectLeft, right: rectRight } = grid.getItemRect(index);

		const { clientWidth: containerWidth, scrollLeft: containerScrollLeft } = container;

		if (rectLeft < containerScrollLeft) {
			// Active item is to the left of the visible area
			container.scrollTo({
				left: rectLeft - 20
			});
		} else if (rectRight > containerScrollLeft + containerWidth) {
			// Active item is to the right of the visible area
			container.scrollTo({
				left: rectRight - containerWidth + 20
			});
		}
	}, [explorer.items, explorer.selectedItems, grid]);

	return (
		<div
			className={clsx(
				'relative mx-auto mt-5 flex w-full flex-row items-center justify-center'
			)}
		>
			<div
				ref={quickPreviewImagesRef}
				onWheel={handleWheel}
				className="quick-preview-images-scroll w-full overflow-x-auto overflow-y-hidden bg-app-lightBox/30 backdrop-blur-md"
			>
				<Grid grid={grid}>
					{(i) => {
						const item = explorer.items?.[i];
						if (!item) return null;
						return (
							<div
								onClick={() => explorer.resetSelectedItems([item])}
								key={i}
								className={clsx(
									'bg-app-lightBox/20',
									'h-full w-full',
									'rounded-md',
									'border',
									explorer.isItemSelected(item)
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
