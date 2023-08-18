import { ReactNode, createContext, useContext, useEffect, useMemo, useRef, useState } from 'react';
import Selecto from 'react-selecto';
import { useKey } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { GridList, useGridList } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { useExplorerContext } from '../Context';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, isCut, useExplorerStore } from '../store';
import { ExplorerItemHash } from '../useExplorer';
import { explorerItemHash } from '../util';

const SelectoContext = createContext<{
	selecto: React.RefObject<Selecto>;
	selectoUnSelected: React.MutableRefObject<Set<ExplorerItemHash>>;
} | null>(null);

type RenderItem = (item: { item: ExplorerItem; selected: boolean; cut: boolean }) => ReactNode;

const GridListItem = (props: {
	index: number;
	item: ExplorerItem;
	children: RenderItem;
	onMouseDown: (e: React.MouseEvent<HTMLDivElement, MouseEvent>) => void;
}) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const selecto = useContext(SelectoContext);

	const cut = isCut(props.item.item.id);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(props.item),
		[explorer.selectedItems, props.item]
	);

	const hash = explorerItemHash(props.item);

	useEffect(() => {
		if (!selecto?.selecto.current || !selecto.selectoUnSelected.current.has(hash)) return;

		if (!selected) {
			selecto.selectoUnSelected.current.delete(hash);
			return;
		}

		const element = document.querySelector(`[data-selectable-id="${hash}"]`);

		if (!element) return;

		selecto.selectoUnSelected.current.delete(hash);
		selecto.selecto.current.setSelectedTargets([
			...selecto.selecto.current.getSelectedTargets(),
			element as HTMLElement
		]);

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	useEffect(() => {
		if (!selecto) return;

		return () => {
			const element = document.querySelector(`[data-selectable-id="${hash}"]`);
			if (selected && !element) selecto.selectoUnSelected.current.add(hash);
		};

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [selected]);

	return (
		<div
			className="h-full w-full"
			data-selectable=""
			data-selectable-index={props.index}
			data-selectable-id={hash}
			onMouseDown={props.onMouseDown}
			onContextMenu={(e) => {
				if (explorerView.selectable && !explorer.selectedItems.has(props.item)) {
					explorer.resetSelectedItems([props.item]);
					selecto?.selecto.current?.setSelectedTargets([e.currentTarget]);
				}
			}}
		>
			{props.children({ item: props.item, selected, cut })}
		</div>
	);
};

const CHROME_REGEX = /Chrome/;

export default ({ children }: { children: RenderItem }) => {
	const os = useOperatingSystem();

	const isChrome = CHROME_REGEX.test(navigator.userAgent);

	const explorer = useExplorerContext();
	const settings = explorer.useSettingsSnapshot();
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const selecto = useRef<Selecto>(null);
	const selectoUnSelected = useRef<Set<ExplorerItemHash>>(new Set());
	const selectoFirstColumn = useRef<number | undefined>();
	const selectoLastColumn = useRef<number | undefined>();

	const [dragFromThumbnail, setDragFromThumbnail] = useState(false);

	const itemDetailsHeight = settings.gridItemSize / 4 + (settings.showBytesInGridView ? 20 : 0);
	const itemHeight = settings.gridItemSize + itemDetailsHeight;

	const grid = useGridList({
		ref: explorerView.ref,
		count: explorer.items?.length ?? 0,
		overscan: explorer.overscan,
		onLoadMore: explorer.loadMore,
		rowsBeforeLoadMore: explorer.rowsBeforeLoadMore,
		size:
			settings.layoutMode === 'grid'
				? { width: settings.gridItemSize, height: itemHeight }
				: undefined,
		columns: settings.layoutMode === 'media' ? settings.mediaColumns : undefined,
		getItemId: (index) => {
			const item = explorer.items?.[index];
			return item ? explorerItemHash(item) : undefined;
		},
		getItemData: (index) => explorer.items?.[index],
		padding: explorerView.padding || settings.layoutMode === 'grid' ? 12 : undefined,
		gap:
			explorerView.gap ||
			(settings.layoutMode === 'grid' ? explorerStore.gridGap : undefined),
		top: explorerView.top
	});

	function getElementId(element: Element) {
		return element.getAttribute('data-selectable-id') as ExplorerItemHash | null;
	}

	function getElementIndex(element: Element) {
		const index = element.getAttribute('data-selectable-index');
		return index ? Number(index) : null;
	}

	function getElementItem(element: Element) {
		const index = getElementIndex(element);
		if (index === null) return null;

		return grid.getItem(index) ?? null;
	}

	useEffect(
		() => {
			const element = explorer.scrollRef.current;
			if (!element) return;

			const handleScroll = () => {
				selecto.current?.checkScroll();
				selecto.current?.findSelectableTargets();
			};

			element.addEventListener('scroll', handleScroll);
			return () => element.removeEventListener('scroll', handleScroll);
		},
		// explorer.scrollRef is a stable reference so this only actually runs once
		[explorer.scrollRef]
	);

	useEffect(() => {
		if (!selecto.current) return;

		const set = new Set(explorer.selectedItemHashes.value);
		if (set.size === 0) return;

		const items = [...document.querySelectorAll('[data-selectable]')].filter((item) => {
			const id = getElementId(item);
			if (id === null) return;

			const selected = set.has(id);
			if (selected) set.delete(id);

			return selected;
		});

		selectoUnSelected.current = set;
		selecto.current.setSelectedTargets(items as HTMLElement[]);

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [grid.columnCount, explorer.items]);

	// The item that further selection will move from (shift + arrow for example).
	// This used to be calculated from the last item of selectedItems,
	// but Set ordering isn't reliable.
	// Ref bc we never actually render this.
	const activeItem = useRef<ExplorerItem | null>(null);

	useEffect(() => {
		if (explorer.selectedItems.size !== 0) return;

		selectoUnSelected.current = new Set();
		// Accessing refs during render is bad
		activeItem.current = null;
	}, [explorer.selectedItems]);

	useKey(['ArrowUp', 'ArrowDown', 'ArrowRight', 'ArrowLeft'], (e) => {
		if (explorer.selectedItems.size > 0) e.preventDefault();

		if (!explorerView.selectable) return;

		const lastItem = activeItem.current;
		if (!lastItem) return;

		const lastItemIndex = explorer.items?.findIndex((item) => item === lastItem);
		if (lastItemIndex === undefined || lastItemIndex === -1) return;

		const gridItem = grid.getItem(lastItemIndex);
		if (!gridItem) return;

		const currentIndex = gridItem.index;
		let newIndex = currentIndex;

		switch (e.key) {
			case 'ArrowUp':
				newIndex -= grid.columnCount;
				break;
			case 'ArrowDown':
				newIndex += grid.columnCount;
				break;
			case 'ArrowRight':
				if (grid.columnCount === (currentIndex % grid.columnCount) + 1) return;
				newIndex += 1;
				break;
			case 'ArrowLeft':
				if (currentIndex % grid.columnCount === 0) return;
				newIndex -= 1;
				break;
		}

		const newSelectedItem = grid.getItem(newIndex);
		if (!newSelectedItem?.data) return;

		if (!explorer.allowMultiSelect) explorer.resetSelectedItems([newSelectedItem.data]);
		else {
			const selectedItemDom = document.querySelector(
				`[data-selectable-id="${explorerItemHash(newSelectedItem.data)}"]`
			);

			if (!selectedItemDom) return;

			if (e.shiftKey) {
				if (!explorer.selectedItems.has(newSelectedItem.data)) {
					explorer.addSelectedItem(newSelectedItem.data);
					selecto.current?.setSelectedTargets([
						...(selecto.current?.getSelectedTargets() || []),
						selectedItemDom as HTMLElement
					]);
				}
			} else {
				explorer.resetSelectedItems([newSelectedItem.data]);
				selecto.current?.setSelectedTargets([selectedItemDom as HTMLElement]);
				if (selectoUnSelected.current.size > 0) selectoUnSelected.current = new Set();
			}
		}

		activeItem.current = newSelectedItem.data;

		if (
			explorer.scrollRef.current &&
			explorerView.ref.current &&
			(e.key === 'ArrowUp' || e.key === 'ArrowDown')
		) {
			const paddingTop = parseInt(getComputedStyle(explorer.scrollRef.current).paddingTop);

			const viewRect = explorerView.ref.current.getBoundingClientRect();

			const itemRect = newSelectedItem.rect;
			const itemTop = itemRect.top + viewRect.top;
			const itemBottom = itemRect.bottom + viewRect.top;

			const scrollRect = explorer.scrollRef.current.getBoundingClientRect();
			const scrollTop = paddingTop + (explorerView.top || 0) + 1;
			const scrollBottom = scrollRect.height - (os !== 'windows' && os !== 'browser' ? 2 : 1);

			if (itemTop < scrollTop) {
				explorer.scrollRef.current.scrollBy({
					top:
						itemTop -
						scrollTop -
						(newSelectedItem.row === 0 ? grid.padding.y : 0) -
						(newSelectedItem.row !== 0 ? grid.gap.y / 2 : 0),
					behavior: 'smooth'
				});
			} else if (itemBottom > scrollBottom) {
				explorer.scrollRef.current.scrollBy({
					top:
						itemBottom -
						scrollBottom +
						(newSelectedItem.row === grid.rowCount - 1 ? grid.padding.y : 0) +
						(newSelectedItem.row !== grid.rowCount - 1 ? grid.gap.y / 2 : 0),
					behavior: 'smooth'
				});
			}
		}
	});

	return (
		<SelectoContext.Provider value={selecto.current ? { selecto, selectoUnSelected } : null}>
			{explorer.allowMultiSelect && (
				<Selecto
					ref={selecto}
					boundContainer={
						explorerView.ref.current
							? {
									element: explorerView.ref.current,
									top: false,
									bottom: false
							  }
							: undefined
					}
					selectableTargets={['[data-selectable]']}
					toggleContinueSelect="shift"
					hitRate={0}
					// selectFromInside={explorerStore.layoutMode === 'media'}
					onDragStart={(e) => {
						getExplorerStore().isDragging = true;
						if ((e.inputEvent as MouseEvent).target instanceof HTMLImageElement) {
							setDragFromThumbnail(true);
						}
					}}
					onDragEnd={() => {
						getExplorerStore().isDragging = false;
						selectoFirstColumn.current = undefined;
						selectoLastColumn.current = undefined;
						setDragFromThumbnail(false);

						const allSelected = selecto.current?.getSelectedTargets() ?? [];

						// Sets active item to selected item with least index.
						// Might seem kinda weird but it's the same behaviour as Finder.
						activeItem.current =
							allSelected.reduce((least, current) => {
								const currentItem = getElementItem(current);
								if (!currentItem) return least;

								if (!least) return currentItem;

								return currentItem.index < least.index ? currentItem : least;
							}, null as ReturnType<typeof getElementItem>)?.data ?? null;
					}}
					onScroll={({ direction }) => {
						selecto.current?.findSelectableTargets();
						explorer.scrollRef.current?.scrollBy(
							(direction[0] || 0) * 10,
							(direction[1] || 0) * 10
						);
					}}
					scrollOptions={{
						container: { current: explorer.scrollRef.current },
						throttleTime: isChrome || dragFromThumbnail ? 30 : 10000
					}}
					onSelect={(e) => {
						const inputEvent = e.inputEvent as MouseEvent;

						if (inputEvent.type === 'mousedown') {
							const el = inputEvent.shiftKey
								? e.added[0] || e.removed[0]
								: e.selected[0];

							if (!el) return;

							const item = getElementItem(el);

							if (!item?.data) return;

							if (!inputEvent.shiftKey) {
								if (explorer.selectedItems.has(item.data)) {
									selecto.current?.setSelectedTargets(e.beforeSelected);
								} else {
									selectoUnSelected.current = new Set();
									explorer.resetSelectedItems([item.data]);
								}

								return;
							}

							if (e.added[0]) explorer.addSelectedItem(item.data);
							else explorer.removeSelectedItem(item.data);
						} else if (inputEvent.type === 'mousemove') {
							const unselectedItems: ExplorerItemHash[] = [];

							e.added.forEach((el) => {
								const item = getElementItem(el);

								if (!item?.data) return;

								explorer.addSelectedItem(item.data);
							});

							e.removed.forEach((el) => {
								const item = getElementItem(el);

								if (!item?.data || typeof item.id === 'number') return;

								if (document.contains(el)) explorer.removeSelectedItem(item.data);
								else unselectedItems.push(item.id);
							});

							const dragDirection = {
								x: inputEvent.x === e.rect.left ? 'left' : 'right',
								y: inputEvent.y === e.rect.bottom ? 'down' : 'up'
							} as const;

							const dragStart = {
								x: dragDirection.x === 'right' ? e.rect.left : e.rect.right,
								y: dragDirection.y === 'down' ? e.rect.top : e.rect.bottom
							};

							const dragEnd = { x: inputEvent.x, y: inputEvent.y };

							const columns = new Set<number>();

							const elements = [...e.added, ...e.removed];

							const items = elements.reduce((items, el) => {
								const item = getElementItem(el);

								if (!item) return items;

								columns.add(item.column);
								return [...items, item];
							}, [] as NonNullable<ReturnType<typeof getElementItem>>[]);

							if (columns.size > 1) {
								items.sort((a, b) => a.column - b.column);

								const firstItem =
									dragDirection.x === 'right'
										? items[0]
										: items[items.length - 1];

								const lastItem =
									dragDirection.x === 'right'
										? items[items.length - 1]
										: items[0];

								if (firstItem && lastItem) {
									selectoFirstColumn.current = firstItem.column;
									selectoLastColumn.current = lastItem.column;
								}
							} else if (columns.size === 1) {
								const column = [...columns.values()][0]!;

								items.sort((a, b) => a.row - b.row);

								const itemRect = elements[0]?.getBoundingClientRect();

								const inDragArea =
									itemRect &&
									(dragDirection.x === 'right'
										? dragEnd.x >= itemRect.left
										: dragEnd.x <= itemRect.right);

								if (
									column !== selectoLastColumn.current ||
									(column === selectoLastColumn.current && !inDragArea)
								) {
									const firstItem =
										dragDirection.y === 'down'
											? items[0]
											: items[items.length - 1];

									if (firstItem) {
										const viewRectTop =
											explorerView.ref.current?.getBoundingClientRect().top ??
											0;

										const itemTop = firstItem.rect.top + viewRectTop;
										const itemBottom = firstItem.rect.bottom + viewRectTop;

										if (
											dragDirection.y === 'down'
												? dragStart.y < itemTop
												: dragStart.y > itemBottom
										) {
											const dragHeight = Math.abs(
												dragStart.y -
													(dragDirection.y === 'down'
														? itemTop
														: itemBottom)
											);

											let itemsInDragCount =
												(dragHeight - grid.gap.y) /
												(grid.virtualItemHeight + grid.gap.y);

											if (itemsInDragCount > 1) {
												itemsInDragCount = Math.ceil(itemsInDragCount);
											} else {
												itemsInDragCount = Math.round(itemsInDragCount);
											}

											[...Array(itemsInDragCount)].forEach((_, i) => {
												const index =
													dragDirection.y === 'down'
														? itemsInDragCount - i
														: i + 1;

												const itemIndex =
													firstItem.index +
													(dragDirection.y === 'down' ? -index : index) *
														grid.columnCount;

												const item = explorer.items?.[itemIndex];

												if (item) {
													if (inputEvent.shiftKey) {
														if (explorer.selectedItems.has(item))
															explorer.removeSelectedItem(item);
														else {
															explorer.addSelectedItem(item);
															if (inDragArea)
																unselectedItems.push(
																	explorerItemHash(item)
																);
														}
													} else if (!inDragArea)
														explorer.removeSelectedItem(item);
													else {
														explorer.addSelectedItem(item);
														if (inDragArea)
															unselectedItems.push(
																explorerItemHash(item)
															);
													}
												}
											});
										}
									}

									if (!inDragArea && column === selectoFirstColumn.current) {
										selectoFirstColumn.current = undefined;
										selectoLastColumn.current = undefined;
									} else {
										selectoLastColumn.current = column;
										if (selectoFirstColumn.current === undefined) {
											selectoFirstColumn.current = column;
										}
									}
								}
							}

							if (unselectedItems.length > 0) {
								selectoUnSelected.current = new Set([
									...selectoUnSelected.current,
									...unselectedItems
								]);
							}
						}
					}}
				/>
			)}

			<GridList grid={grid} scrollRef={explorer.scrollRef}>
				{(index) => {
					const item = explorer.items?.[index];

					if (!item) return null;

					return (
						<GridListItem
							index={index}
							item={item}
							onMouseDown={(e) => {
								e.stopPropagation();

								if (!explorerView.selectable) return;

								const item = grid.getItem(index);

								if (!item?.data) return;

								if (!explorer.allowMultiSelect) {
									explorer.resetSelectedItems([item.data]);
								} else {
									selectoFirstColumn.current = item.column;
									selectoLastColumn.current = item.column;
								}

								activeItem.current = item.data;
							}}
						>
							{children}
						</GridListItem>
					);
				}}
			</GridList>
		</SelectoContext.Provider>
	);
};
