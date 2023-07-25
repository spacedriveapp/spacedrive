import {
	ReactNode,
	createContext,
	memo,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState
} from 'react';
import Selecto from 'react-selecto';
import { useKey } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { GridList, GridListItem as GridListItemType, useGridList } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, isCut, useExplorerStore } from '../store';

const SelectoContext = createContext<{
	selecto: React.RefObject<Selecto>;
	selectoUnSelected: React.MutableRefObject<Set<number>>;
} | null>(null);

const useSelectoContext = () => useContext(SelectoContext);

type RenderItem = (item: { item: ExplorerItem; selected: boolean; cut: boolean }) => ReactNode;

const GridListItem = (props: {
	index: number;
	item: ExplorerItem;
	children: RenderItem;
	onMouseDown: () => void;
}) => {
	const explorerView = useExplorerViewContext();
	const selecto = useSelectoContext();

	const cut = isCut(props.item.item.id);

	const selected = useMemo(
		() =>
			typeof explorerView.selected === 'object'
				? explorerView.selected.has(props.item.item.id)
				: explorerView.selected === props.item.item.id,
		[explorerView.selected, props.item.item.id]
	);

	useEffect(() => {
		if (!selecto) return;

		if (selecto.selecto.current && selecto.selectoUnSelected.current.has(props.item.item.id)) {
			if (selected) {
				const element = document.querySelector(
					`[data-selectable-id="${props.item.item.id}"]`
				);
				if (element) {
					selecto.selectoUnSelected.current.delete(props.item.item.id);
					selecto.selecto.current.setSelectedTargets([
						...selecto.selecto.current.getSelectedTargets(),
						element as HTMLElement
					]);
				}
			} else {
				selecto.selectoUnSelected.current.delete(props.item.item.id);
			}
		}

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	useEffect(() => {
		if (!selecto) return;

		return () => {
			const element = document.querySelector(`[data-selectable-id="${props.item.item.id}"]`);
			if (selected && !element) selecto.selectoUnSelected.current.add(props.item.item.id);
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

				if (explorerView.onSelectedChange && typeof explorerView.selected !== 'object') {
					explorerView.onSelectedChange(props.item.item.id);
				}
			}}
			onContextMenu={(e) => {
				if (explorerView.contextMenu !== undefined && explorerView.onSelectedChange) {
					if (typeof explorerView.selected === 'object') {
						if (!explorerView.selected.has(props.item.item.id)) {
							explorerView.onSelectedChange(new Set([props.item.item.id]));
							selecto?.selecto.current?.setSelectedTargets([e.currentTarget]);
						}
					} else explorerView.onSelectedChange(props.item.item.id);
				}
			}}
		>
			{props.children({ item: props.item, selected, cut })}
		</div>
	);
};

export default ({ children }: { children: RenderItem }) => {
	const os = useOperatingSystem();

	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const selecto = useRef<Selecto>(null);
	const selectoUnSelected = useRef<Set<number>>(new Set());
	const selectoLastColumn = useRef<number | undefined>();

	const itemDetailsHeight =
		explorerStore.gridItemSize / 4 + (explorerStore.showBytesInGridView ? 20 : 0);
	const itemHeight = explorerStore.gridItemSize + itemDetailsHeight;

	const grid = useGridList({
		ref: explorerView.viewRef,
		count: explorerView.items ? explorerView.items.length : 0,
		size:
			explorerStore.layoutMode === 'grid'
				? { width: explorerStore.gridItemSize, height: itemHeight }
				: undefined,
		columns: explorerStore.layoutMode === 'media' ? explorerStore.mediaColumns : undefined,
		getItemId: (index) => explorerView.items?.[index]?.item.id,
		padding: explorerView.padding || explorerStore.layoutMode === 'grid' ? 12 : undefined,
		gap: explorerView.gap || explorerStore.layoutMode === 'grid' ? 24 : undefined,
		overscan: explorerView.overscan,
		onLoadMore: explorerView.onLoadMore,
		rowsBeforeLoadMore: explorerView.rowsBeforeLoadMore,
		top: explorerView.top
	});

	function getItemId(item: Element) {
		const id = item.getAttribute('data-selectable-id');
		return id ? Number(id) : null;
	}

	function getItemIndex(item: Element) {
		const index = item.getAttribute('data-selectable-index');
		return index ? Number(index) : null;
	}

	function getItem(element: Element) {
		const index = getItemIndex(element);
		const item = index !== null ? grid.getItem(index) : undefined;
		return item;
	}

	useEffect(() => {
		const element = explorerView.scrollRef.current;
		if (!element) return;

		const handleScroll = () => {
			selecto.current?.checkScroll();
			selecto.current?.findSelectableTargets();
		};

		element.addEventListener('scroll', handleScroll);
		return () => element.removeEventListener('scroll', handleScroll);
	}, [explorerView.scrollRef]);

	useEffect(() => {
		if (!selecto.current || typeof explorerView.selected !== 'object') return;

		const set = new Set(explorerView.selected);
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
	}, [grid.columnCount, explorerView.items]);

	useEffect(() => {
		if (typeof explorerView.selected !== 'object' || explorerView.selected.size !== 0) return;

		selectoUnSelected.current = new Set();
	}, [explorerView.selected]);

	useKey(['ArrowUp', 'ArrowDown', 'ArrowRight', 'ArrowLeft'], (e) => {
		if (
			explorerView.onSelectedChange &&
			(typeof explorerView.selected === 'object'
				? explorerView.selected.size > 0
				: explorerView.selected !== undefined)
		) {
			e.preventDefault();
		}

		if (!explorerView.selectable || !explorerView.onSelectedChange) return;

		const lastItemId =
			typeof explorerView.selected === 'object'
				? Array.from(explorerView.selected)[explorerView.selected.size - 1]
				: explorerView.selected;

		if (!lastItemId) return;

		const lastItemIndex = explorerView.items?.findIndex((item) => item.item.id === lastItemId);
		if (lastItemIndex === undefined || lastItemIndex === -1) return;

		const lastItem = grid.getItem(lastItemIndex);
		if (!lastItem) return;

		const currentIndex = lastItem.index;
		let newIndex = currentIndex;

		switch (e.key) {
			case 'ArrowUp':
				newIndex += -grid.columnCount;
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
				newIndex += -1;
				break;
		}

		const newSelectedItem = grid.getItem(newIndex);
		if (!newSelectedItem) return;

		if (typeof explorerView.selected !== 'object') {
			explorerView.onSelectedChange(newSelectedItem.id);
		} else {
			const addToGridListSelection = e.shiftKey;

			const selectedItemDom = document.querySelector(
				`[data-selectable-id="${newSelectedItem.id}"]`
			) as HTMLElement;

			if (addToGridListSelection) {
				const set = new Set(explorerView.selected);

				if (set.has(newSelectedItem.id)) {
					set.delete(newSelectedItem.id);
					set.add(newSelectedItem.id);
					explorerView.onSelectedChange(set);
				} else {
					set.add(newSelectedItem.id);
					explorerView.onSelectedChange(set);
					selecto.current?.setSelectedTargets([
						...(selecto.current?.getSelectedTargets() || []),
						selectedItemDom
					]);
				}
			} else {
				explorerView.onSelectedChange(new Set([newSelectedItem.id]));
				selecto.current?.setSelectedTargets([selectedItemDom]);
				if (selectoUnSelected.current.size > 0) selectoUnSelected.current = new Set();
			}
		}

		if (
			explorerView.scrollRef.current &&
			explorerView.viewRef.current &&
			(e.key === 'ArrowUp' || e.key === 'ArrowDown')
		) {
			const paddingTop = parseInt(
				getComputedStyle(explorerView.scrollRef.current).paddingTop
			);

			const viewRect = explorerView.viewRef.current.getBoundingClientRect();

			const itemRect = newSelectedItem.rect;
			const itemTop = itemRect.top + viewRect.top;
			const itemBottom = itemRect.bottom + viewRect.top;

			const scrollRect = explorerView.scrollRef.current.getBoundingClientRect();
			const scrollTop = paddingTop + (explorerView.top || 0) + 1;
			const scrollBottom = scrollRect.height - (os !== 'windows' && os !== 'browser' ? 2 : 1);

			if (itemTop < scrollTop) {
				explorerView.scrollRef.current.scrollBy({
					top:
						itemTop -
						scrollTop -
						(newSelectedItem.row === 0 ? grid.padding.y : 0) -
						(newSelectedItem.row !== 0 ? grid.gap.y / 2 : 0),
					behavior: 'smooth'
				});
			} else if (itemBottom > scrollBottom) {
				explorerView.scrollRef.current.scrollBy({
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
			{typeof explorerView.selected === 'object' && (
				<Selecto
					ref={selecto}
					boundContainer={
						explorerView.viewRef.current && {
							element: explorerView.viewRef.current,
							top: false,
							bottom: false
						}
					}
					selectableTargets={['[data-selectable]']}
					toggleContinueSelect="shift"
					hitRate={0}
					// selectFromInside={explorerStore.layoutMode === 'media'}
					onDragStart={() => {
						getExplorerStore().isDragging = true;
					}}
					onDragEnd={() => {
						getExplorerStore().isDragging = false;
						selectoLastColumn.current = undefined;
					}}
					onScroll={({ direction }) => {
						selecto.current?.findSelectableTargets();
						explorerView.scrollRef.current?.scrollBy(
							direction[0]! * 10,
							direction[1]! * 10
						);
					}}
					scrollOptions={{
						container: explorerView.scrollRef.current!,
						throttleTime: 10000
					}}
					onSelect={(e) => {
						explorerView.onSelectedChange?.((selected) => {
							if (typeof selected !== 'object') return new Set();

							const set = new Set(selected);

							const inputEvent = e.inputEvent as MouseEvent;

							if (inputEvent.type === 'mousedown') {
								const el = inputEvent.shiftKey
									? e.added[0] || e.removed[0]
									: e.selected[0];

								if (el) {
									const item = getItem(el);

									if (item) {
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
											return new Set([item.id]);
										}

										if (e.added[0]) set.add(item.id);
										else set.delete(item.id);
									}
								}
							}

							if (inputEvent.type === 'mousemove') {
								const unselectedItems: number[] = [];

								e.added.forEach((el) => {
									const id = getItemId(el);
									if (id !== null) set.add(id);
								});

								e.removed.forEach((el) => {
									const id = getItemId(el);
									if (id !== null) {
										if (document.contains(el)) set.delete(id);
										else unselectedItems.push(id);
									}
								});

								const dragDirectionX =
									inputEvent.x === e.rect.left ? 'left' : 'right';

								const dragDirectionY =
									inputEvent.y === e.rect.bottom ? 'down' : 'up';

								const dragStartX =
									dragDirectionX === 'right' ? e.rect.left : e.rect.right;

								const dragEndX = inputEvent.x;

								const dragStartY =
									dragDirectionY === 'down' ? e.rect.top : e.rect.bottom;

								const dragEndY = inputEvent.y;

								const columns = new Set<number>();

								const elements = [...e.added, ...e.removed];

								const items = elements.reduce((items, el) => {
									const item = el && getItem(el);
									if (item) {
										columns.add(item.column);
										return [...items, item];
									}

									return items;
								}, [] as GridListItemType[]);

								if (columns.size > 1 && selectoLastColumn.current === undefined) {
									items.sort((a, b) => a.column - b.column);

									const lastItem =
										dragDirectionX === 'right'
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
											(dragDirectionX === 'right'
												? dragEndX >= itemRect.left
												: dragEndX <= itemRect.right);

										if (
											column !== selectoLastColumn.current ||
											(column === selectoLastColumn.current && !inDragArea)
										) {
											const firstItem =
												dragDirectionY === 'down'
													? items[0]
													: items[items.length - 1];

											if (firstItem) {
												const viewRect =
													explorerView.viewRef.current?.getBoundingClientRect();

												const dragHeight = Math.abs(
													dragStartY -
														((dragDirectionY === 'down'
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
														dragDirectionY === 'down'
															? itemsInDragCount - i
															: i + 1;

													const itemIndex =
														firstItem.index +
														(dragDirectionY === 'down'
															? -index
															: index) *
															grid.columnCount;

													const id =
														explorerView.items?.[itemIndex]?.item.id;

													if (id !== null && id !== undefined) {
														if (inputEvent.shiftKey) {
															if (set.has(id)) set.delete(id);
															else {
																set.add(id);
																if (inDragArea) {
																	unselectedItems.push(id);
																}
															}
														} else if (!inDragArea) {
															set.delete(id);
														} else {
															set.add(id);
															if (inDragArea) {
																unselectedItems.push(id);
															}
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

							return set;
						});
					}}
				/>
			)}

			<GridList grid={grid} scrollRef={explorerView.scrollRef}>
				{(index) => {
					const item = explorerView.items?.[index];

					if (!item) return null;

					return (
						<GridListItem
							index={index}
							item={item}
							onMouseDown={() => {
								const item = grid.getItem(index);
								if (item) selectoLastColumn.current = item.column;
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
