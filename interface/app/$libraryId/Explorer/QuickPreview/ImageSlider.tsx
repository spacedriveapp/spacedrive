import { Grid, useGrid } from '@virtual-grid/react';
import clsx from 'clsx';
import { useCallback, useEffect, useRef, useState } from 'react';
import { ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { uniqueId } from '../util';

export const ImageSlider = () => {
	const quickPreviewImagesRef = useRef<HTMLDivElement>(null);
	const explorer = useExplorerContext();
	const [activeIndex, setActiveIndex] = useState<null | number>(0);
	const [isScrolling, setIsScrolling] = useState(false);
	const [isUsingKeyboard, setIsUsingKeyboard] = useState(false);

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

	const containerScrollHandler = useCallback(() => {
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
				left: itemLeft - 20
			});
		} else if (itemRight > containerScrollLeft + containerWidth) {
			// Active item is to the right of the visible area
			container.scrollTo({
				left: itemRight - containerWidth + 20
			});
		}
	}, [activeIndex, grid]);

	const selectHandler = (i: number) => {
		if (activeIndex === null) return;
		const item = explorer.items?.[i];
		if (!item) return;
		explorer.resetSelectedItems([item]);
		setIsScrolling(false);
		setActiveIndex(i);
	};

	const activeItem = (item: ExplorerItem): boolean => {
		const [selectedItem] = Array.from(explorer.selectedItems);
		if (!selectedItem) return false;
		const getIndex = explorer.items?.findIndex((i) => {
			if (selectedItem.item) {
				return uniqueId(i) === uniqueId(selectedItem);
			}
			return false;
		});
		if (getIndex === undefined) return false;
		setActiveIndex(getIndex);
		return uniqueId(item) === uniqueId(selectedItem);
	};

	// Scroll to the active item on initial render
	useEffect(() => {
		if (activeIndex === null || isScrolling) return;
		containerScrollHandler();
	}, [activeIndex, containerScrollHandler, isScrolling]);

	useEffect(() => {
		const keyboardTimer = setTimeout(() => {
			setIsUsingKeyboard(false);
		}, 500);

		const handleKeyBoardState = (e: KeyboardEvent) => {
			if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
				setIsUsingKeyboard(true);
			}
		};

		if (isUsingKeyboard) containerScrollHandler();

		//cleaning this up breaks the functionality - must be as it is
		document.addEventListener('keydown', handleKeyBoardState);

		return () => {
			clearTimeout(keyboardTimer);
		};
	}, [containerScrollHandler, isUsingKeyboard]);

	const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
		const container = quickPreviewImagesRef.current;
		if (!container) return;

		const delta = e.deltaY || e.deltaX;
		container.scrollLeft += delta;

		// Prevent the default wheel behavior to avoid unwanted scrolling
		e.preventDefault();
	};

	return (
		<div
			className={clsx(
				'relative mx-auto mb-4 flex w-fit flex-row items-center justify-center md:max-w-[600px] lg:max-w-[700px]',
				'rounded-md '
			)}
		>
			<div
				ref={quickPreviewImagesRef}
				onScroll={() => setIsScrolling(true)}
				onWheel={handleWheel}
				className="quick-preview-images-scroll w-full overflow-x-auto overflow-y-hidden rounded-md bg-app-lightBox/30 backdrop-blur-md"
			>
				<Grid grid={grid}>
					{(i) => {
						const item = explorer.items?.[i];
						if (!item) return null;
						return (
							<div
								onClick={(e) => selectHandler(i)}
								key={i}
								className={clsx(
									'bg-app-lightBox/20',
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
