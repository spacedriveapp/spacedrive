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
import { useBoundingclientrect, useIntersectionObserverRef, useKey } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { TOP_BAR_HEIGHT } from '~/app/$libraryId/TopBar';

interface Props {
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
	selectable?: boolean;
	overscan?: number;
	onLastRow?: () => void;
}
interface WrapSizing extends Props {
	size: number | { width: number; height: number };
}

interface ResizeSizing extends Props {
	columns: number;
}

export default ({ selectable = true, ...props }: WrapSizing | ResizeSizing) => {
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
	const getBoundingClientRect = useBoundingclientrect(ref);

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
	}, [getBoundingClientRect]);

	useEffect(() => {
		const lastRow = virtualRows[virtualRows.length - 1];
		if (lastRow?.index === amountOfRows - 1) props.onLastRow?.();
	}, [virtualRows]);

	const handleArrowSelection = (nextIndex: number, add: boolean = true) => {
		if (selecto.current) {
			const selected = selecto.current.getSelectedTargets();
			const lastItem = selected[selected.length - 1];
			if (lastItem) {
				const index = Number(lastItem.getAttribute('data-selectable-index'));

				const next = document.querySelector<HTMLDivElement>(
					`[data-selectable-index="${index + nextIndex}"]`
				);

				if (next && props.scrollRef.current) {
					const direction = nextIndex > 0 ? 'down' : 'up';

					const eleBound = next.getBoundingClientRect();
					const scrollBound = props.scrollRef.current.getBoundingClientRect();

					switch (direction) {
						case 'up': {
							if (eleBound.top < 0) {
								const paddingTop = parseInt(
									getComputedStyle(props.scrollRef.current).paddingTop
								);
								props.scrollRef.current.scrollBy({
									top: eleBound.top - paddingTop - (paddingY || 0),
									behavior: 'smooth'
								});
							}
							break;
						}
						case 'down': {
							if (eleBound.bottom > scrollBound.height) {
								props.scrollRef.current.scrollBy({
									top: eleBound.bottom - scrollBound.height + (paddingY || 0),
									behavior: 'smooth'
								});
							}
							break;
						}
					}

					selecto.current.setSelectedTargets([...(add ? selected : []), next]);
					props.onSelectedChange?.(
						new Set(
							[...(add ? selected : []), next].map((el) =>
								Number(el.getAttribute('data-selectable-id'))
							)
						)
					);

					if (!add) {
						document
							.querySelectorAll('[data-selected="true"]')
							.forEach((el) => el.setAttribute('data-selected', 'false'));
					}
					next.setAttribute('data-selected', 'true');
				}
			}
		}
	};

	useKey('ArrowUp', (e) => {
		e.preventDefault();
		handleArrowSelection(-amountOfColumns, e.shiftKey);
	});
	useKey('ArrowDown', (e) => {
		e.preventDefault();
		handleArrowSelection(amountOfColumns, e.shiftKey);
	});
	useKey('ArrowRight', (e) => handleArrowSelection(1, e.shiftKey));
	useKey('ArrowLeft', (e) => handleArrowSelection(-1, e.shiftKey));

	return (
		<>
			{selectable && (
				<Selecto
					ref={selecto}
					dragContainer={props.scrollRef.current}
					boundContainer={props.scrollRef.current}
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
			)}

			<div
				ref={ref}
				className="relative w-full"
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`
				}}
			>
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
