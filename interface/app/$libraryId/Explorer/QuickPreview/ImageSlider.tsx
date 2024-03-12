import { Grid, useGrid } from '@virtual-grid/react';
import clsx from 'clsx';
import { memo, useCallback, useEffect, useMemo, useRef } from 'react';
import { ExplorerItem, getExplorerItemData } from '@sd/client';
import { Tooltip } from '@sd/ui';

import { QuickPreviewItem } from '.';
import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';

export const ImageSlider = ({ activeItem }: { activeItem: QuickPreviewItem }) => {
	const explorer = useExplorerContext();

	const ref = useRef<HTMLDivElement>(null);
	const activeIndex = useRef<number | null>(null);

	const grid = useGrid({
		scrollRef: ref,
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

	const handleWheel = (event: React.WheelEvent<HTMLDivElement>) => {
		const element = ref.current;
		if (!element) return;

		event.preventDefault();

		const { deltaX, deltaY } = event;
		const scrollAmount = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;

		element.scrollTo({ left: element.scrollLeft + scrollAmount });
	};

	useEffect(() => {
		const element = ref.current;
		if (!element) return;

		const { index } = activeItem;
		if (index === activeIndex.current) return;

		const gridItem = grid.getItem(index);
		if (!gridItem) return;

		const { left: rectLeft, right: rectRight, width: rectWidth } = gridItem.rect;

		const { clientWidth: containerWidth, scrollLeft: containerScrollLeft } = element;

		if (activeIndex.current === null) {
			const scrollTo = rectLeft - containerWidth / 2 + rectWidth / 2;
			// Initial scroll needs to be done in a timeout otherwise it won't scroll
			setTimeout(() => element.scrollTo({ left: scrollTo }));
		} else if (rectLeft < containerScrollLeft) {
			element.scrollTo({ left: rectLeft - 20 });
		} else if (rectRight > containerScrollLeft + containerWidth) {
			element.scrollTo({ left: rectRight - containerWidth + 20 });
		}

		activeIndex.current = index;
	}, [activeItem, explorer.items, explorer.selectedItems, grid]);

	return (
		<div
			ref={ref}
			onWheel={handleWheel}
			className="quick-preview-images-scroll mt-5 overflow-x-auto overflow-y-hidden bg-app-lightBox/30 backdrop-blur-md"
		>
			<Grid grid={grid}>
				{(i) => {
					const item = explorer.items?.[i];
					if (!item) return null;
					return <Image key={i} item={item} active={activeItem.item === item} />;
				}}
			</Grid>
		</div>
	);
};

const Image = memo(({ item, active }: { item: ExplorerItem; active: boolean }) => {
	const explorer = useExplorerContext();

	const { fullName } = getExplorerItemData(item);

	const selected = useMemo(
		() => explorer.selectedItems.has(item),
		[explorer.selectedItems, item]
	);

	return (
		<Tooltip tooltipClassName="!break-all" label={fullName} position="top">
			<div
				onClick={() => explorer.resetSelectedItems([item])}
				className={clsx(
					'relative size-full rounded-md border bg-app-lightBox/20',
					explorer.selectedItems.size > 1 && !selected && 'opacity-25',
					selected ? 'border-2 border-accent' : 'border-1 border-white/5',
					selected && !active && 'border-white/5'
				)}
			>
				<FileThumb data={item} />
			</div>
		</Tooltip>
	);
});
