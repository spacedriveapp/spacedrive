import { useEffect, useRef } from 'react';
import Selecto from 'react-selecto';
import { useKey } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { GridList, useGridList } from '~/components';
import { useExplorerViewContext } from '../ViewContext';
import { getExplorerStore, useExplorerStore } from '../store';

interface Props {
	children: (item: ExplorerItem) => JSX.Element;
}

export default (props: Props) => {
	const explorerStore = useExplorerStore();
	const explorerView = useExplorerViewContext();

	const selecto = useRef<Selecto>(null);
	const selectoUnSelected = useRef<Set<number>>(new Set());

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
		getItemData: (index) => explorerView.items?.[index],
		getItemId: (item) => item.item.id,
		padding: explorerView.padding || explorerStore.layoutMode === 'grid' ? 12 : undefined,
		overscan: explorerView.overscan,
		onLoadMore: explorerView.onLoadMore,
		rowsBeforeLoadMore: explorerView.rowsBeforeLoadMore,
		top: explorerView.top
	});

	function getItemId(item: Element) {
		const id = item.getAttribute('data-selectable-id');
		return id ? Number(id) : null;
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
	}, [grid.amountOfColumns]);

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

		const { items, itemsById } = grid.getGridItems();

		const lastItem = itemsById[lastItemId];
		if (!lastItem) return;

		const currentIndex = lastItem.index;
		let newIndex = currentIndex;

		switch (e.key) {
			case 'ArrowUp':
				newIndex += -grid.amountOfColumns;
				break;
			case 'ArrowDown':
				newIndex += grid.amountOfColumns;
				break;
			case 'ArrowRight':
				if (grid.amountOfColumns === (currentIndex % grid.amountOfColumns) + 1) return;
				newIndex += 1;
				break;
			case 'ArrowLeft':
				if (currentIndex % grid.amountOfColumns === 0) return;
				newIndex += -1;
				break;
		}

		const newSelectedItem = items[newIndex];
		if (!newSelectedItem) return;

		if (typeof explorerView.selected !== 'object') {
			explorerView.onSelectedChange(newSelectedItem.id as number);
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
			const scrollTop = paddingTop + (explorerView.top || 0);
			const scrollBottom = scrollRect.bottom;

			if (itemTop < scrollTop) {
				explorerView.scrollRef.current.scrollBy({
					top: itemTop - scrollTop - (newSelectedItem.row === 0 ? grid.padding.y + 1 : 0),
					behavior: 'smooth'
				});
			} else if (itemBottom > scrollBottom) {
				explorerView.scrollRef.current.scrollBy({
					top:
						itemBottom -
						scrollBottom +
						(newSelectedItem.row === grid.amountOfRows - 1 ? grid.padding.y + 1 : 0),
					behavior: 'smooth'
				});
			}
		}
	});

	return (
		<>
			{typeof explorerView.selected === 'object' && (
				<Selecto
					ref={selecto}
					boundContainer={explorerView.viewRef.current}
					selectableTargets={['[data-selectable]']}
					toggleContinueSelect="shift"
					hitRate={0}
					selectFromInside={false}
					onDragStart={() => {
						getExplorerStore().isDragging = true;
					}}
					onDragEnd={() => {
						getExplorerStore().isDragging = false;
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
								if (inputEvent.shiftKey) {
									const [added] = e.added;
									const addedId = added ? getItemId(added) : null;
									if (addedId !== null) set.add(addedId);

									const [removed] = e.removed;
									const removedId = removed ? getItemId(removed) : null;
									if (removedId !== null) set.delete(removedId);

									return set;
								} else {
									const [selectedItem] = e.selected;

									const id = selectedItem ? getItemId(selectedItem) : null;

									if (selectedItem && id !== null) {
										if (set.has(id)) {
											selecto.current?.setSelectedTargets(e.beforeSelected);
											return set;
										} else {
											selecto.current?.setSelectedTargets([selectedItem]);
											if (selectoUnSelected.current.size > 0) {
												selectoUnSelected.current = new Set();
											}
											return new Set([id]);
										}
									}
								}
							} else if (inputEvent.type === 'mousemove') {
								const notRemoved: number[] = [];

								e.removed.forEach((el) => {
									const id = getItemId(el);
									if (id !== null) {
										if (document.contains(el)) set.delete(id);
										else notRemoved.push(id);
									}
								});

								if (notRemoved.length > 0) {
									selectoUnSelected.current = new Set([
										...selectoUnSelected.current,
										...notRemoved
									]);
								}

								e.added.forEach((el) => {
									const id = getItemId(el);
									if (id !== null) set.add(id);
								});
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

					if (selecto.current && selectoUnSelected.current.has(item.item.id)) {
						const element = document.querySelector(
							`[data-selectable-id="${item.item.id}"]`
						);
						if (element) {
							selectoUnSelected.current.delete(item.item.id);
							selecto.current.setSelectedTargets([
								...selecto.current.getSelectedTargets(),
								element as HTMLElement
							]);
						}
					}

					return (
						<div
							className="h-full w-full"
							data-selectable=""
							data-selectable-id={item.item.id}
							onMouseDown={(e) => {
								e.stopPropagation();

								if (
									explorerView.onSelectedChange &&
									typeof explorerView.selected !== 'object'
								) {
									explorerView.onSelectedChange(item.item.id);
								}
							}}
							onContextMenu={(e) => {
								if (
									explorerView.contextMenu !== undefined &&
									explorerView.onSelectedChange
								) {
									if (typeof explorerView.selected === 'object') {
										if (!explorerView.selected.has(item.item.id)) {
											explorerView.onSelectedChange(new Set([item.item.id]));
											selecto.current?.setSelectedTargets([e.currentTarget]);
										}
									} else explorerView.onSelectedChange(item.item.id);
								}
							}}
						>
							{props.children(item)}
						</div>
					);
				}}
			</GridList>
		</>
	);
};
