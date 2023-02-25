import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useKey, useOnWindowResize } from 'rooks';
import { ExplorerContext, ExplorerItem, isPath } from '@sd/client';
import { ExplorerLayoutMode, getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { LIST_VIEW_HEADER_HEIGHT, ListViewHeader } from './FileColumns';
import FileItem from './FileItem';
import FileRow from './FileRow';

const TOP_BAR_HEIGHT = 46;
// const GRID_TEXT_AREA_HEIGHT = 25;

interface Props {
	context: ExplorerContext;
	data: ExplorerItem[];
	onScroll?: (posY: number) => void;
}

export const VirtualizedList = memo(({ data, context, onScroll }: Props) => {
	const scrollRef = useRef<HTMLDivElement>(null);
	const innerRef = useRef<HTMLDivElement>(null);

	const [goingUp, setGoingUp] = useState(false);
	const [width, setWidth] = useState(0);

	const explorerStore = useExplorerStore();

	function handleWindowResize() {
		// so the virtualizer can render the correct number of columns
		setWidth(innerRef.current?.offsetWidth || 0);
	}
	useOnWindowResize(handleWindowResize);
	useLayoutEffect(() => handleWindowResize(), []);
	useEffect(() => {
		setWidth(innerRef.current?.offsetWidth || 0);
	}, [explorerStore.showInspector]);

	// sizing calculations
	const GRID_TEXT_AREA_HEIGHT = explorerStore.gridItemSize / 4;
	const amountOfColumns = Math.floor(width / explorerStore.gridItemSize) || 4,
		amountOfRows =
			explorerStore.layoutMode === 'grid' ? Math.ceil(data.length / amountOfColumns) : data.length,
		itemSize =
			explorerStore.layoutMode === 'grid'
				? explorerStore.gridItemSize + GRID_TEXT_AREA_HEIGHT
				: explorerStore.listItemSize;

	useEffect(() => {
		const el = scrollRef.current;
		if (!el) return;

		const onElementScroll = (event: Event) => {
			onScroll?.((event.target as HTMLElement).scrollTop);
		};

		el.addEventListener('scroll', onElementScroll);

		return () => el.removeEventListener('scroll', onElementScroll);
	}, [onScroll]);

	const rowVirtualizer = useVirtualizer({
		count: amountOfRows,
		getScrollElement: () => scrollRef.current,
		overscan: 200,
		estimateSize: () => itemSize,
		measureElement: (index) => itemSize
	});

	// TODO: Make scroll adjustment work with both list and grid layout, currently top bar offset disrupts positioning of list, and grid just doesn't work
	// useEffect(() => {
	// 	if (selectedRowIndex === 0 && goingUp) rowVirtualizer.scrollToIndex(0, { smoothScroll: false });

	// 	if (selectedRowIndex !== -1)
	// 		rowVirtualizer.scrollToIndex(goingUp ? selectedRowIndex - 1 : selectedRowIndex, {
	// 			smoothScroll: false
	// 		});
	// }, [goingUp, selectedRowIndex, rowVirtualizer]);

	useKey('ArrowUp', (e) => {
		e.preventDefault();
		setGoingUp(true);
		if (explorerStore.selectedRowIndex !== -1 && explorerStore.selectedRowIndex !== 0)
			getExplorerStore().selectedRowIndex = explorerStore.selectedRowIndex - 1;
	});

	useKey('ArrowDown', (e) => {
		e.preventDefault();
		setGoingUp(false);
		if (
			explorerStore.selectedRowIndex !== -1 &&
			explorerStore.selectedRowIndex !== (data.length ?? 1) - 1
		)
			getExplorerStore().selectedRowIndex = explorerStore.selectedRowIndex + 1;
	});

	return (
		<div style={{ marginTop: -TOP_BAR_HEIGHT }} className="w-full cursor-default pl-4">
			<div
				ref={scrollRef}
				className="custom-scroll explorer-scroll h-screen"
				onClick={(e) => {
					getExplorerStore().selectedRowIndex = -1;
				}}
			>
				<div
					ref={innerRef}
					style={{
						height: `${rowVirtualizer.getTotalSize()}px`,
						marginTop: `${TOP_BAR_HEIGHT + LIST_VIEW_HEADER_HEIGHT}px`
					}}
					className="relative w-full"
				>
					<ListViewHeader />
					{rowVirtualizer.getVirtualItems().map((virtualRow) => (
						<div
							style={{
								height: `${virtualRow.size}px`,
								transform: `translateY(${virtualRow.start}px)`
							}}
							className="absolute top-0 left-0 flex w-full"
							key={virtualRow.key}
						>
							{explorerStore.layoutMode === 'list' && (
								<WrappedItem
									kind="list"
									isSelected={explorerStore.selectedRowIndex === virtualRow.index}
									index={virtualRow.index}
									item={data[virtualRow.index]!}
								/>
							)}
							{explorerStore.layoutMode === 'grid' &&
								[...Array(amountOfColumns)].map((_, i) => {
									const index = virtualRow.index * amountOfColumns + i;
									const item = data[index];
									const isSelected = explorerStore.selectedRowIndex === index;
									return (
										<div key={index} className="flex">
											{item && (
												<WrappedItem
													kind="grid"
													isSelected={isSelected}
													index={index}
													item={item}
												/>
											)}
										</div>
									);
								})}
						</div>
					))}
				</div>
			</div>
		</div>
	);
});

interface WrappedItemProps {
	item: ExplorerItem;
	index: number;
	isSelected: boolean;
	kind: ExplorerLayoutMode;
}

// Wrap either list item or grid item with click logic as it is the same for both
const WrappedItem = memo(({ item, index, isSelected, kind }: WrappedItemProps) => {
	const [_, setSearchParams] = useSearchParams();

	const onDoubleClick = useCallback(() => {
		if (isPath(item) && item.item.is_dir) setSearchParams({ path: item.item.materialized_path });
	}, [item, setSearchParams]);

	const onClick = useCallback(
		(e: React.MouseEvent<HTMLDivElement>) => {
			e.stopPropagation();
			getExplorerStore().selectedRowIndex = isSelected ? -1 : index;
		},
		[isSelected, index]
	);

	const ItemComponent = kind === 'list' ? FileRow : FileItem;

	return (
		<ItemComponent
			data={item}
			index={index}
			onClick={onClick}
			onDoubleClick={onDoubleClick}
			selected={isSelected}
		/>
	);
});
