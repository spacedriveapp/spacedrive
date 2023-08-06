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

const useSelectoContext = () => {
	const ctx = useContext(SelectoContext);

	if (!ctx) throw new Error('Selecto context not found');

	return ctx;
};

type RenderItem = (item: { item: ExplorerItem; selected: boolean; cut: boolean }) => ReactNode;

const GridListItem = (props: {
	index: number;
	item: ExplorerItem;
	children: RenderItem;
	onMouseDown: () => void;
}) => {
	const explorer = useExplorerContext();

	const selecto = useSelectoContext();

	const cut = isCut(props.item.item.id);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(props.item),
		[explorer.selectedItems, props.item]
	);

	const hash = explorerItemHash(props.item);

	useEffect(() => {
		if (!selecto.selecto.current || !selecto.selectoUnSelected.current.has(hash)) return;

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
			data-selectable-id={props.item.item.id}
			onMouseDown={(e) => {
				e.stopPropagation();

				props.onMouseDown();

				explorer.resetSelectedItems();
				explorer.addSelectedItem(props.item);
			}}
			onContextMenu={(e) => {
				if (!explorer.selectedItems.has(props.item)) {
					explorer.resetSelectedItems();
					explorer.addSelectedItem(props.item);
					selecto?.selecto.current?.setSelectedTargets([e.currentTarget]);
				}
			}}
		>
			{props.children({ item: props.item, selected, cut })}
		</div>
	);
};

export default ({ children }: { children: RenderItem }) => {
	const os = useOperatingSystem();

	const isChrome = /Chrome/.test(navigator.userAgent);

	const explorer = useExplorerContext();
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const selecto = useRef<Selecto>(null);
	const selectoUnSelected = useRef<Set<ExplorerItemHash>>(new Set());
	const selectoLastColumn = useRef<number | undefined>();

	const [dragFromThumbnail, setDragFromThumbnail] = useState(false);

	const itemDetailsHeight =
		explorerStore.gridItemSize / 4 + (explorerStore.showBytesInGridView ? 20 : 0);
	const itemHeight = explorerStore.gridItemSize + itemDetailsHeight;

	const grid = useGridList({
		ref: explorerView.ref,
		count: explorer.items?.length ?? 0,
		size:
			explorerStore.layoutMode === 'grid'
				? { width: explorerStore.gridItemSize, height: itemHeight }
				: undefined,
		columns: explorerStore.layoutMode === 'media' ? explorerStore.mediaColumns : undefined,
		getItemId: (index) => {
			const item = explorer.items?.[index];
			return item ? explorerItemHash(item) : undefined;
		},
		getItemData: (index) => {
			return explorer.items?.[index];
		},
		padding: explorerView.padding || explorerStore.layoutMode === 'grid' ? 12 : undefined,
		gap:
			explorerView.gap ||
			(explorerStore.layoutMode === 'grid' ? explorerStore.gridGap : undefined),
		overscan: explorerView.overscan,
		onLoadMore: explorer.loadMore,
		rowsBeforeLoadMore: explorer.rowsBeforeLoadMore,
		top: explorerView.top
	});

	function getItemId(item: Element) {
		return item.getAttribute('data-selectable-id') as ExplorerItemHash | null;
	}

	function getItemIndex(item: Element) {
		const index = item.getAttribute('data-selectable-index');
		return index ? Number(index) : null;
	}

	function getItem(element: Element) {
		const index = getItemIndex(element);
		if (index === null) return null;

		return grid.getItem(index) ?? null;
	}

	useEffect(() => {
		const element = explorer.scrollRef.current;
		if (!element) return;

		const handleScroll = () => {
			selecto.current?.checkScroll();
			selecto.current?.findSelectableTargets();
		};

		element.addEventListener('scroll', handleScroll);
		return () => element.removeEventListener('scroll', handleScroll);
	}, [explorer.scrollRef]);

	useEffect(() => {
		if (!selecto.current) return;

		const set = new Set(explorer.selectedItemHashes.value);
		if (set.size === 0) return;

		const items = [...document.querySelectorAll('[data-selectable]')].filter((item) => {
			const id = getItemId(item);
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
		if (!newSelectedItem) return;

		if (!explorer.allowMultiSelect) explorer.resetSelectedItems([newSelectedItem.data]);
		else {
			const addToGridListSelection = e.shiftKey;

			const selectedItemDom = document.querySelector(
				`[data-selectable-id="${newSelectedItem.id}"]`
			) as HTMLElement;

			if (addToGridListSelection) {
				if (!explorer.selectedItems.has(newSelectedItem.data)) {
					explorer.addSelectedItem(newSelectedItem.data);
					selecto.current?.setSelectedTargets([
						...(selecto.current?.getSelectedTargets() || []),
						selectedItemDom
					]);
				}
			} else {
				explorer.resetSelectedItems([newSelectedItem.data]);
				selecto.current?.setSelectedTargets([selectedItemDom]);
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
						selectoLastColumn.current = undefined;
						setDragFromThumbnail(false);

						const allSelected = selecto.current?.getSelectedTargets() ?? [];

						// Sets active item to selected item with least index.
						// Might seem kinda weird but it's the same behaviour as Finder.
						activeItem.current =
							allSelected.reduce((least, current) => {
								const currentItem = getItem(current);
								if (!currentItem) return least;

								if (!least) return currentItem;

								return currentItem.index < least.index ? currentItem : least;
							}, null as ReturnType<typeof getItem>)?.data ?? null;
					}}
					onScroll={({ direction }) => {
						selecto.current?.findSelectableTargets();
						explorer.scrollRef.current?.scrollBy(
							(direction[0] || 0) * 10,
							(direction[1] || 0) * 10
						);
					}}
					scrollOptions={{
						container: explorer.scrollRef.current!,
						throttleTime: isChrome || dragFromThumbnail ? 30 : 10000
					}}
					onSelect={(e) => {
						const inputEvent = e.inputEvent as MouseEvent;

						if (inputEvent.type === 'mousedown') {
							const el = inputEvent.shiftKey
								? e.added[0] || e.removed[0]
								: e.selected[0];

							if (!el) return;

							const item = getItem(el);

							if (!item) return;

							selectoLastColumn.current = item.column;

							if (!inputEvent.shiftKey) {
								// TODO: Uncomment when implementing dnd
								// if (set.has(item.id)) {
								// 	selecto.current?.setSelectedTargets(
								// 		e.beforeSelected
								// 	);
								// 	return set;
								// } else {
								// 	selecto.current?.setSelectedTargets([el]);
								// 	selectoUnSelected.current = new Set();
								// 	return new Set([item.id]);
								// }

								selectoUnSelected.current = new Set();
								explorer.resetSelectedItems([item.data]);
							}

							if (e.added[0]) explorer.addSelectedItem(item.data);
							else explorer.removeSelectedItem(item.data);
						} else if (inputEvent.type === 'mousemove') {
							const unselectedItems: ExplorerItemHash[] = [];

							e.added.forEach((el) => {
								const item = getItem(el);
								if (!item) return;
								explorer.addSelectedItem(item.data);
							});

							e.removed.forEach((el) => {
								const item = getItem(el);
								if (!item) return;

								if (document.contains(el)) explorer.removeSelectedItem(item.data);
								else unselectedItems.push(item.id);
							});

							const dragDirection = {
								x: inputEvent.x === e.rect.left ? 'left' : 'right',
								y: inputEvent.y === e.rect.bottom ? 'down' : 'up'
							};

							const dragStart = {
								x: dragDirection.x === 'right' ? e.rect.left : e.rect.right,
								y: dragDirection.y === 'down' ? e.rect.top : e.rect.bottom
							};

							const dragEnd = { x: inputEvent.x, y: inputEvent.y };

							const columns = new Set<number>();

							const elements = [...e.added, ...e.removed];

							const items = elements.reduce((items, el) => {
								const item = el && getItem(el);

								if (!item) return items;

								columns.add(item.column);
								return [...items, item];
							}, [] as NonNullable<ReturnType<typeof getItem>>[]);

							if (columns.size > 1 && selectoLastColumn.current === undefined) {
								items.sort((a, b) => a.column - b.column);

								const lastItem =
									dragDirection.x === 'right'
										? items[items.length - 1]
										: items[0];

								if (lastItem) selectoLastColumn.current = lastItem.column;
							} else if (columns.size === 1) {
								const column = [...columns.values()][0];

								if (column !== undefined) {
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
											const viewRect =
												explorerView.ref.current?.getBoundingClientRect();

											const dragHeight = Math.abs(
												dragStart.y -
													((dragDirection.y === 'down'
														? firstItem.rect.top
														: firstItem.rect.bottom) +
														(viewRect?.top || 0))
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

										if (
											!inDragArea &&
											(column === 0 || column === grid.columnCount - 1)
										) {
											selectoLastColumn.current = undefined;
										} else {
											selectoLastColumn.current = column;
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
							onMouseDown={() => {
								const item = grid.getItem(index);

								if (!item) return;

								selectoLastColumn.current = item.column;
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
