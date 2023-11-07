import { Grid, useGrid } from '@virtual-grid/react';
import { memo, ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import Selecto from 'react-selecto';
import { type ExplorerItem } from '@sd/client';
import { useMouseNavigate, useOperatingSystem, useShortcut } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { getQuickPreviewStore } from '../../QuickPreview/store';
import { getExplorerStore, useExplorerStore } from '../../store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../../ViewContext';
import { GridContext } from './context';
import { GridItem } from './Item';

export type RenderItem = (item: {
	item: ExplorerItem;
	selected: boolean;
	cut: boolean;
}) => ReactNode;

const CHROME_REGEX = /Chrome/;

export default memo(({ children }: { children: RenderItem }) => {
	const os = useOperatingSystem();
	const realOS = useOperatingSystem(true);
	const mouseNavigate = useMouseNavigate();

	const isChrome = CHROME_REGEX.test(navigator.userAgent);

	const explorer = useExplorerContext();
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();
	const settings = explorer.useSettingsSnapshot();

	const selecto = useRef<Selecto>(null);
	const selectoUnselected = useRef<Set<string>>(new Set());
	const selectoFirstColumn = useRef<number | undefined>();
	const selectoLastColumn = useRef<number | undefined>();

	// The item that further selection will move from (shift + arrow for example).
	// This used to be calculated from the last item of selectedItems,
	// but Set ordering isn't reliable.
	// Ref bc we never actually render this.
	const activeItem = useRef<ExplorerItem | null>(null);

	const [dragFromThumbnail, setDragFromThumbnail] = useState(false);

	const itemDetailsHeight = 44 + (settings.showBytesInGridView ? 20 : 0);
	const itemHeight = settings.gridItemSize + itemDetailsHeight;
	const padding = settings.layoutMode === 'grid' ? 12 : 0;

	const grid = useGrid({
		scrollRef: explorer.scrollRef,
		count: explorer.items?.length ?? 0,
		totalCount: explorer.count,
		...(settings.layoutMode === 'grid'
			? { size: { width: settings.gridItemSize, height: itemHeight } }
			: { columns: settings.mediaColumns }),
		rowVirtualizer: { overscan: explorer.overscan ?? 5 },
		onLoadMore: explorer.loadMore,
		getItemId: useCallback(
			(index: number) => {
				const item = explorer.items?.[index];
				return item ? uniqueId(item) : undefined;
			},
			[explorer.items]
		),
		getItemData: useCallback((index: number) => explorer.items?.[index], [explorer.items]),
		padding: {
			...explorerView.padding,
			bottom: explorerView.bottom
				? (explorerView.padding?.bottom ?? padding) + explorerView.bottom
				: undefined,
			x: padding,
			y: padding
		},
		gap: explorerView.gap || (settings.layoutMode === 'grid' ? settings.gridGap : 1)
	});

	function getElementId(element: Element) {
		return element.getAttribute('data-selectable-id');
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

	function getActiveItem(elements: Element[]) {
		// Get selected item with least index.
		// Might seem kinda weird but it's the same behaviour as Finder.
		const activeItem =
			elements.reduce(
				(least, current) => {
					const currentItem = getElementItem(current);
					if (!currentItem) return least;

					if (!least) return currentItem;

					return currentItem.index < least.index ? currentItem : least;
				},
				null as ReturnType<typeof getElementItem>
			)?.data ?? null;

		return activeItem;
	}

	function handleDragEnd() {
		getExplorerStore().isDragSelecting = false;
		selectoFirstColumn.current = undefined;
		selectoLastColumn.current = undefined;
		setDragFromThumbnail(false);

		const allSelected = selecto.current?.getSelectedTargets() ?? [];
		activeItem.current = getActiveItem(allSelected);
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

		selectoUnselected.current = set;
		selecto.current.setSelectedTargets(items as HTMLElement[]);

		activeItem.current = getActiveItem(items);

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [grid.columnCount, explorer.items]);

	useEffect(() => {
		if (explorer.selectedItems.size !== 0) return;

		selectoUnselected.current = new Set();
		// Accessing refs during render is bad
		activeItem.current = null;
	}, [explorer.selectedItems]);

	useShortcut('explorerEscape', () => {
		if (!explorerView.selectable) return;
		explorer.resetSelectedItems([]);
		selecto.current?.setSelectedTargets([]);
	});

	const keyboardHandler = (e: KeyboardEvent, newIndex: number) => {
		if (!explorerView.selectable) return;

		if (explorer.selectedItems.size > 0) e.preventDefault();

		const lastItem = activeItem.current;
		if (!lastItem) return;

		const lastItemIndex = explorer.items?.findIndex((item) => item === lastItem);
		if (lastItemIndex === undefined || lastItemIndex === -1) return;

		const gridItem = grid.getItem(lastItemIndex);
		if (!gridItem) return;

		const currentIndex = gridItem.index;
		let updatedIndex = currentIndex;
		updatedIndex = newIndex;
		const newSelectedItem = grid.getItem(updatedIndex);
		if (!newSelectedItem?.data) return;
		if (!explorer.allowMultiSelect) explorer.resetSelectedItems([newSelectedItem.data]);
		else {
			const id = uniqueId(newSelectedItem.data);

			const selectedItemDom = document.querySelector(
				`[data-selectable-id="${realOS === 'windows' ? id.replaceAll('\\', '\\\\') : id}"]`
			);
			if (!selectedItemDom) return;

			if (e.shiftKey && !getQuickPreviewStore().open) {
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
				if (selectoUnselected.current.size > 0) selectoUnselected.current = new Set();
			}
		}

		activeItem.current = newSelectedItem.data;

		if (
			explorer.scrollRef.current &&
			explorerView.ref.current &&
			newSelectedItem.row !== gridItem.row
		) {
			const viewRect = explorerView.ref.current.getBoundingClientRect();

			const itemRect = newSelectedItem.rect;
			const itemTop = itemRect.top + viewRect.top;
			const itemBottom = itemRect.bottom + viewRect.top;

			const scrollRect = explorer.scrollRef.current.getBoundingClientRect();
			const scrollTop =
				(explorerView.top ??
					parseInt(getComputedStyle(explorer.scrollRef.current).paddingTop)) + 1;
			const scrollBottom = scrollRect.height - (os !== 'windows' && os !== 'browser' ? 2 : 1);

			if (itemTop < scrollTop) {
				explorer.scrollRef.current.scrollBy({
					top:
						itemTop -
						scrollTop -
						(newSelectedItem.row === 0 ? grid.padding.top : grid.gap.y / 2),
					behavior: 'smooth'
				});
			} else if (itemBottom > scrollBottom - (explorerView.bottom ?? 0)) {
				explorer.scrollRef.current.scrollBy({
					top:
						itemBottom -
						scrollBottom +
						(explorerView.bottom ?? 0) +
						(newSelectedItem.row === grid.rowCount - 1
							? grid.padding.bottom
							: grid.gap.y / 2),
					behavior: 'smooth'
				});
			}
		}
	};
	const getGridItemHandler = (key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight') => {
		const lastItem = activeItem.current;
		if (!lastItem) return;

		const lastItemIndex = explorer.items?.findIndex((item) => item === lastItem);
		if (lastItemIndex === undefined || lastItemIndex === -1) return;

		const gridItem = grid.getItem(lastItemIndex);
		if (!gridItem) return;

		let newIndex = gridItem.index;

		switch (key) {
			case 'ArrowUp':
				newIndex -= grid.columnCount;
				break;
			case 'ArrowDown':
				newIndex += grid.columnCount;
				break;
			case 'ArrowLeft':
				newIndex -= 1;
				break;
			case 'ArrowRight':
				newIndex += 1;
				break;
		}
		return newIndex;
	};

	useShortcut('explorerDown', (e) => {
		if (!explorerView.selectable) return;
		if (explorer.selectedItems.size === 0) {
			const item = grid.getItem(0);
			if (!item?.data) return;

			const id = uniqueId(item.data);

			const selectedItemDom = document.querySelector(
				`[data-selectable-id="${realOS === 'windows' ? id.replaceAll('\\', '\\\\') : id}"]`
			);

			if (selectedItemDom) {
				explorer.resetSelectedItems([item.data]);
				selecto.current?.setSelectedTargets([selectedItemDom as HTMLElement]);
				activeItem.current = item.data;
			}
		} else {
			const newIndex = getGridItemHandler('ArrowDown');
			if (newIndex === undefined) return;
			keyboardHandler(e, newIndex);
		}
	});

	useShortcut('explorerUp', (e) => {
		const newIndex = getGridItemHandler('ArrowUp');
		if (newIndex === undefined) return;
		keyboardHandler(e, newIndex);
	});

	useShortcut('explorerLeft', (e) => {
		const newIndex = getGridItemHandler('ArrowLeft');
		if (newIndex === undefined) return;
		keyboardHandler(e, newIndex);
	});

	useShortcut('explorerRight', (e) => {
		const newIndex = getGridItemHandler('ArrowRight');
		if (newIndex === undefined) return;
		keyboardHandler(e, newIndex);
	});

	return (
		<GridContext.Provider value={{ selecto, selectoUnselected }}>
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
					onDrag={(e) => {
						if (!explorerStore.drag) return;
						e.stop();
						handleDragEnd();
					}}
					onDragStart={({ inputEvent }) => {
						getExplorerStore().isDragSelecting = true;

						if ((inputEvent as MouseEvent).target instanceof HTMLImageElement) {
							setDragFromThumbnail(true);
						}
					}}
					onDragEnd={handleDragEnd}
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
									selectoUnselected.current = new Set();
									explorer.resetSelectedItems([item.data]);
								}

								return;
							}

							if (e.added[0]) explorer.addSelectedItem(item.data);
							else explorer.removeSelectedItem(item.data);
						} else if (inputEvent.type === 'mousemove') {
							const unselectedItems: string[] = [];

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

							const items = elements.reduce(
								(items, el) => {
									const item = getElementItem(el);

									if (!item) return items;

									columns.add(item.column);
									return [...items, item];
								},
								[] as NonNullable<ReturnType<typeof getElementItem>>[]
							);

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
												(grid.virtualItemSize.height + grid.gap.y);

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
																	uniqueId(item)
																);
														}
													} else if (!inDragArea)
														explorer.removeSelectedItem(item);
													else {
														explorer.addSelectedItem(item);
														if (inDragArea)
															unselectedItems.push(uniqueId(item));
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
								selectoUnselected.current = new Set([
									...selectoUnselected.current,
									...unselectedItems
								]);
							}
						}
					}}
				/>
			)}

			<Grid grid={grid}>
				{(index) => {
					const item = explorer.items?.[index];
					if (!item) return null;

					return (
						<GridItem
							index={index}
							item={item}
							onMouseDown={(e) => {
								e.stopPropagation();

								mouseNavigate(e);

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
						</GridItem>
					);
				}}
			</Grid>
		</GridContext.Provider>
	);
});
