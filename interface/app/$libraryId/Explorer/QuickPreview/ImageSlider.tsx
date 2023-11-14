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
				if (getIndex === undefined) return false;
				setActiveIndex(getIndex);
				return selectedItem.item.name === item.item.name;
			}
			return false;
		},
		[explorer.selectedItems, explorer.items]
	);

	//on initial load, scroll to the active item
	useEffect(() => {
		containerScrollHandler();
	});

	useEffect(() => {
		let scrollTimer: ReturnType<typeof setTimeout>;
		let keyboardTimer: ReturnType<typeof setTimeout>;
		const handleScrollStart = () => {
			setIsScrolling(true);
			clearTimeout(scrollTimer);
			scrollTimer = setTimeout(() => {
				setIsScrolling(false);
			}, 500);
		};

		const handleKeyBoardState = (e: KeyboardEvent) => {
			if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
				setIsUsingKeyboard(true);
			}
			clearTimeout(keyboardTimer);
			keyboardTimer = setTimeout(() => {
				setIsUsingKeyboard(false);
			}, 500);
		};

		//we don't want to clean up the event listeners as they are being listened to by the grid
		document.addEventListener('scroll', handleScrollStart);
		document.addEventListener('keydown', handleKeyBoardState);

		if (!isScrolling || !isUsingKeyboard) containerScrollHandler();
	}, [containerScrollHandler, isScrolling, isUsingKeyboard]);

	return (
		<div
			className={clsx(
				'relative mx-auto mb-4 flex w-fit max-w-[700px] flex-row items-center justify-center',
				'rounded-md '
			)}
		>
			<div
				ref={quickPreviewImagesRef}
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
