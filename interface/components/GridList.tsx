import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import React, {
	HTMLAttributes,
	PropsWithChildren,
	ReactNode,
	cloneElement,
	createContext,
	useContext,
	useRef
} from 'react';
import { RefObject, useEffect, useMemo, useState } from 'react';
import Selecto, { SelectoProps } from 'react-selecto';
import { useBoundingclientrect, useIntersectionObserverRef, useKey, useKeys } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { TOP_BAR_HEIGHT } from '~/app/$libraryId/TopBar';

interface GridListDefaults {
	count: number;
	scrollRef: RefObject<HTMLElement>;
	padding?: number | { x?: number; y?: number };
	gap?: number | { x?: number; y?: number };
	children: (props: {
		index: number;
		item: (props: GridListItemProps) => JSX.Element;
	}) => JSX.Element | null;
	selected?: Set<number>;
	onSelectedChange?: (change: Set<number>) => void;
	onSelect?: (index: number) => void;
	onDeselect?: (index: number) => void;
	overscan?: number;
	top?: number;
	onLoadMore?: () => void;
	rowsBeforeLoadMore?: number;
}
interface WrapProps extends GridListDefaults {
	size: number | { width: number; height: number };
}

interface ResizeProps extends GridListDefaults {
	columns: number;
}

export default (props: WrapProps | ResizeProps) => {
	const scrollBarWidth = 6;

	const paddingX = (typeof props.padding === 'object' ? props.padding.x : props.padding) || 0;
	const paddingY = (typeof props.padding === 'object' ? props.padding.y : props.padding) || 0;

	const gapX = (typeof props.gap === 'object' ? props.gap.x : props.gap) || 0;
	const gapY = (typeof props.gap === 'object' ? props.gap.y : props.gap) || 0;

	const itemWidth =
		'size' in props
			? typeof props.size === 'object'
				? props.size.width
				: props.size
			: undefined;

	const itemHeight =
		'size' in props
			? typeof props.size === 'object'
				? props.size.height
				: props.size
			: undefined;

	const ref = useRef<HTMLDivElement>(null);
	const { width = 0 } = useResizeObserver({ ref: ref });
	const rect = useBoundingclientrect(ref);

	const selecto = useRef<Selecto>(null);

	const [scrollOptions, setScrollOptions] = React.useState<SelectoProps['scrollOptions']>();
	const [listOffset, setListOffset] = useState(0);

	// console.log('list offset: ', listOffset);

	const gridWidth = width - (paddingX || 0) * 2;

	// Virtualizer count calculation
	let amountOfColumns =
		'columns' in props ? props.columns : itemWidth ? Math.floor(gridWidth / itemWidth) : 0;
	const amountOfRows = amountOfColumns > 0 ? Math.ceil(props.count / amountOfColumns) : 0;

	// Virtualizer item size calculation
	const virtualItemWidth = amountOfColumns > 0 ? gridWidth / amountOfColumns : 0;
	const virtualItemHeight = itemHeight || virtualItemWidth;

	const rowVirtualizer = useVirtualizer({
		count: amountOfRows,
		getScrollElement: () => props.scrollRef.current,
		estimateSize: () => virtualItemHeight,
		measureElement: () => virtualItemHeight,
		paddingStart: paddingY,
		paddingEnd: paddingY,
		overscan: props.overscan,
		scrollMargin: listOffset
	});

	const columnVirtualizer = useVirtualizer({
		horizontal: true,
		count: amountOfColumns,
		getScrollElement: () => props.scrollRef.current,
		estimateSize: () => virtualItemWidth,
		measureElement: () => virtualItemWidth,
		paddingStart: paddingX,
		paddingEnd: paddingX
	});

	const virtualRows = rowVirtualizer.getVirtualItems();
	const virtualColumns = columnVirtualizer.getVirtualItems();

	// Measure virtual item on size change
	useEffect(() => {
		rowVirtualizer.measure();
		columnVirtualizer.measure();
	}, [rowVirtualizer, columnVirtualizer, virtualItemWidth, virtualItemHeight]);

	// Set Selecto scroll options
	useEffect(() => {
		setScrollOptions({
			container: props.scrollRef.current!,
			getScrollPosition: () => {
				return [props.scrollRef.current?.scrollLeft!, props.scrollRef.current?.scrollTop!];
			},
			throttleTime: 30,
			threshold: 0
		});
	}, []);

	// Check Selecto scroll
	useEffect(() => {
		const handleScroll = () => {
			selecto.current?.checkScroll();
		};

		props.scrollRef.current?.addEventListener('scroll', handleScroll);
		return () => props.scrollRef.current?.removeEventListener('scroll', handleScroll);
	}, []);

	useEffect(() => {
		setListOffset(ref.current?.offsetTop || 0);
	}, [rect]);

	// Handle key selection
	useKey(['ArrowUp', 'ArrowDown', 'ArrowRight', 'ArrowLeft'], (e) => {
		e.preventDefault();

		const selectedItems = selecto.current?.getSelectedTargets() || [
			...document.querySelectorAll<HTMLDivElement>(`[data-selected="true"]`)
		];
		const lastItem = selectedItems[selectedItems.length - 1];

		if (lastItem) {
			const currentIndex = Number(lastItem.getAttribute('data-selectable-index'));
			let newIndex = currentIndex;

			switch (e.key) {
				case 'ArrowUp':
					newIndex += -amountOfColumns;
					break;
				case 'ArrowDown':
					newIndex += amountOfColumns;
					break;
				case 'ArrowRight':
					newIndex += 1;
					break;
				case 'ArrowLeft':
					newIndex += -1;
					break;
			}

			const newSelectedItem = document.querySelector<HTMLDivElement>(
				`[data-selectable-index="${newIndex}"]`
			);

			if (newSelectedItem) {
				const addToSelection = e.shiftKey;

				selecto.current?.setSelectedTargets([
					...(addToSelection ? selectedItems : []),
					newSelectedItem
				]);

				props.onSelectedChange?.(
					new Set(
						[...(addToSelection ? selectedItems : []), newSelectedItem].map((el) =>
							Number(el.getAttribute('data-selectable-id'))
						)
					)
				);

				if (!addToSelection) {
					document
						.querySelectorAll('[data-selected="true"]')
						.forEach((el) => el.setAttribute('data-selected', 'false'));
				}

				newSelectedItem.setAttribute('data-selected', 'true');

				if (props.scrollRef.current) {
					const direction = newIndex > currentIndex ? 'down' : 'up';

					const itemRect = newSelectedItem.getBoundingClientRect();
					const scrollRect = props.scrollRef.current.getBoundingClientRect();

					const paddingTop = parseInt(
						getComputedStyle(props.scrollRef.current).paddingTop
					);

					const top = props.top ? paddingTop + props.top : paddingTop;

					switch (direction) {
						case 'up': {
							if (itemRect.top < top) {
								props.scrollRef.current.scrollBy({
									top: itemRect.top - top - paddingY,
									behavior: 'smooth'
								});
							}
							break;
						}
						case 'down': {
							if (itemRect.bottom > scrollRect.height) {
								props.scrollRef.current.scrollBy({
									top: itemRect.bottom - scrollRect.height + paddingY,
									behavior: 'smooth'
								});
							}
							break;
						}
					}
				}
			}
		}
	});

	useEffect(() => {
		if (props.onLoadMore) {
			const lastRow = virtualRows[virtualRows.length - 1];
			if (lastRow) {
				const rowsBeforeLoadMore = props.rowsBeforeLoadMore || 1;

				const loadMoreOnIndex =
					rowsBeforeLoadMore > amountOfRows ||
					lastRow.index > amountOfRows - rowsBeforeLoadMore
						? amountOfRows - 1
						: amountOfRows - rowsBeforeLoadMore;

				if (lastRow.index === loadMoreOnIndex) props.onLoadMore();
			}
		}
	}, [virtualRows, amountOfRows, props.rowsBeforeLoadMore, props.onLoadMore]);

	return (
		<>
			<div
				ref={ref}
				className="relative w-full"
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`
				}}
			>
				<Selecto
					ref={selecto}
					dragContainer={ref.current}
					boundContainer={ref.current}
					selectableTargets={['[data-selectable]']}
					toggleContinueSelect={'shift'}
					hitRate={0}
					scrollOptions={scrollOptions}
					onDragStart={(e) => {
						if (e.inputEvent.target.nodeName === 'BUTTON') {
							return false;
						}
						return true;
					}}
					onScroll={(e) => {
						selecto.current;
						props.scrollRef.current?.scrollBy(
							e.direction[0]! * 10,
							e.direction[1]! * 10
						);
					}}
					onSelect={(e) => {
						// console.log(e);

						const set = new Set(props.selected);

						e.removed.forEach((el) => {
							el.setAttribute('data-selected', 'false');
							set.delete(Number(el.getAttribute('data-selectable-id')));
							// const id = Number(el.getAttribute('data-selectable-id'));
							// props.onDeselect?.(id);
							// props.onSelectedChange?.();
						});

						e.added.forEach((el) => {
							el.setAttribute('data-selected', 'true');
							set.add(Number(el.getAttribute('data-selectable-id')));
							// const id = Number(el.getAttribute('data-selectable-id'));
							// props.onSelect?.(id);
						});

						props.onSelectedChange?.(set);
					}}
				/>

				{width !== 0 && (
					<SelectoContext.Provider value={selecto}>
						{virtualRows.map((virtualRow) => (
							<React.Fragment key={virtualRow.index}>
								{virtualColumns.map((virtualColumn) => {
									const index =
										virtualRow.index * amountOfColumns + virtualColumn.index;
									const item = props.children({ index, item: GridListItem });

									if (!item) return null;
									return (
										<div
											key={virtualColumn.index}
											style={{
												position: 'absolute',
												top: 0,
												left: 0,
												width: `${virtualColumn.size}px`,
												height: `${virtualRow.size}px`,
												transform: `translateX(${
													virtualColumn.start
												}px) translateY(${
													virtualRow.start -
													rowVirtualizer.options.scrollMargin
												}px)`
											}}
										>
											{cloneElement<GridListItemProps>(item, {
												style: { width: itemWidth }
											})}
										</div>
									);
								})}
							</React.Fragment>
						))}
					</SelectoContext.Provider>
				)}
			</div>
		</>
	);
};

const SelectoContext = createContext<React.RefObject<Selecto>>(undefined!);
const useSelecto = () => useContext(SelectoContext);

interface GridListItemDefaults
	extends Omit<HTMLAttributes<HTMLDivElement>, 'id'>,
		PropsWithChildren {}

interface NonSelectableGridListItemProps extends GridListItemDefaults {
	selectable?: false;
}

interface SelectableGridListItemProps extends GridListItemDefaults {
	selectable: true;
	index: number;
	selected: boolean;
	id?: number;
}

type GridListItemProps = NonSelectableGridListItemProps | SelectableGridListItemProps;

const GridListItem = ({ className, children, style, ...props }: GridListItemProps) => {
	const ref = useRef<HTMLDivElement>(null);
	const selecto = useSelecto();

	useEffect(() => {
		if (props.selectable && props.selected && selecto.current) {
			console.log('new');
			const current = selecto.current.getSelectedTargets();

			selecto.current?.setSelectedTargets([
				...current.filter(
					(el) => el.getAttribute('data-selectable-id') !== String(props.id)
				),
				ref.current!
			]);
		}
	}, []);

	const selectableProps = props.selectable
		? {
				'data-selectable': '',
				'data-selectable-id': props.id || props.index,
				'data-selectable-index': props.index,
				'data-selected': props.selected
		  }
		: {};

	return (
		<div
			ref={ref}
			{...selectableProps}
			style={style}
			className={clsx('mx-auto h-full', className)}
		>
			{children}
		</div>
	);
};
