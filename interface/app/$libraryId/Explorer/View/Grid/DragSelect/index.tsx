import { useGrid } from '@virtual-grid/react';
import { PropsWithChildren, useEffect, useRef } from 'react';
import Selecto, { SelectoEvents } from 'react-selecto';

import { ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../../../Context';
import { explorerStore } from '../../../store';
import { useExplorerOperatingSystem } from '../../../useExplorerOperatingSystem';
import { useExplorerViewContext } from '../../Context';
import { DragSelectContext } from './context';
import { useSelectedTargets } from './useSelectedTargets';
import { getElementIndex, SELECTABLE_DATA_ATTRIBUTE } from './util';

const CHROME_REGEX = /Chrome/;

type GridOpts = ReturnType<typeof useGrid<string, ExplorerItem | undefined>>;

interface Props extends PropsWithChildren {
	columnCount: GridOpts['columnCount'];
	gapY: GridOpts['gap']['y'];
	getItem: GridOpts['getItem'];
	totalColumnCount: GridOpts['totalColumnCount'];
	totalCount: GridOpts['totalCount'];
	totalRowCount: GridOpts['totalRowCount'];
	virtualItemHeight: GridOpts['virtualItemHeight'];
}

export interface Drag {
	startColumn: number;
	endColumn: number;
	startRow: number;
	endRow: number;
}

export const DragSelect = ({ children, ...props }: Props) => {
	const isChrome = CHROME_REGEX.test(navigator.userAgent);

	const { explorerOperatingSystem, matchingOperatingSystem } = useExplorerOperatingSystem();

	const isWindows = explorerOperatingSystem === 'windows' && matchingOperatingSystem;

	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const selecto = useRef<Selecto>(null);

	const drag = useRef<Drag | null>(null);

	const selectedTargets = useSelectedTargets(selecto);

	useEffect(() => {
		if (explorer.selectedItems.size !== 0) return;
		selectedTargets.resetSelectedTargets();

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [explorer.selectedItems, selectedTargets.resetSelectedTargets]);

	useEffect(() => {
		const node = explorer.scrollRef.current;
		if (!node) return;

		const handleScroll = () => {
			selecto.current?.checkScroll();
			selecto.current?.findSelectableTargets();
		};

		node.addEventListener('scroll', handleScroll);
		return () => node.removeEventListener('scroll', handleScroll);
	}, [explorer.scrollRef]);

	function getGridItem(element: Element) {
		const index = getElementIndex(element);
		return (index !== null && props.getItem(index)) || undefined;
	}

	function handleScroll(e: SelectoEvents['scroll']) {
		selecto.current?.findSelectableTargets();
		explorer.scrollRef.current?.scrollBy(
			(e.direction[0] || 0) * 10,
			(e.direction[1] || 0) * 10
		);
	}

	function handleDrag(e: SelectoEvents['drag']) {
		if (!explorerStore.drag) return;
		e.stop();
		handleDragEnd();
	}

	function handleDragStart(_: SelectoEvents['dragStart']) {
		explorerStore.isDragSelecting = true;
	}

	function handleDragEnd() {
		explorerStore.isDragSelecting = false;

		const dragState = drag.current;
		drag.current = null;

		// Determine if the drag event was a click
		if (
			dragState?.startColumn === dragState?.endColumn &&
			dragState?.startRow === dragState?.endRow
		) {
			return;
		}

		// Update active item to the first selected target(first grid item in DOM).
		const target = selecto.current?.getSelectedTargets()?.[0];

		const item = target && getGridItem(target);
		if (!item) return;

		explorerView.updateActiveItem(item.id as string, {
			updateFirstItem: true
		});
	}

	function handleSelect(e: SelectoEvents['select']) {
		const inputEvent = e.inputEvent as MouseEvent;

		const continueSelection =
			inputEvent.shiftKey || (isWindows ? inputEvent.ctrlKey : inputEvent.metaKey);

		// Handle select on mouse down
		if (inputEvent.type === 'mousedown') {
			const element = continueSelection ? e.added[0] || e.removed[0] : e.selected[0];
			if (!element) return;

			const item = getGridItem(element);
			if (!item?.data) return;

			drag.current = {
				startColumn: item.column,
				endColumn: item.column,
				startRow: item.row,
				endRow: item.row
			};

			if (!continueSelection) {
				if (
					explorerOperatingSystem !== 'windows' &&
					explorer.selectedItems.has(item.data)
				) {
					// Keep previous selection as selecto will reset it otherwise
					selecto.current?.setSelectedTargets(e.beforeSelected);
				} else {
					explorer.resetSelectedItems([item.data]);
					selectedTargets.resetSelectedTargets([
						{ id: String(item.id), node: element as HTMLElement }
					]);
				}

				explorerView.updateActiveItem(item.id as string, { updateFirstItem: true });
				return;
			}

			if (explorerOperatingSystem === 'windows' && inputEvent.shiftKey) {
				explorerView.handleWindowsGridShiftSelection(item.index);
				return;
			}

			if (e.added[0]) {
				explorer.addSelectedItem(item.data);
				explorerView.updateActiveItem(item.id as string, { updateFirstItem: true });
				return;
			}

			explorer.removeSelectedItem(item.data);

			explorerView.updateActiveItem(
				explorerOperatingSystem === 'windows' ? (item.id as string) : null,
				{
					updateFirstItem: true
				}
			);

			return;
		}

		// Handle select by drag
		if (inputEvent.type === 'mousemove') {
			// Collect all elements from the drag event
			// that are still in the DOM
			const elements: Element[] = [];

			for (const element of e.added) {
				const item = getGridItem(element);
				if (!item?.data) continue;

				// Add item to selected targets
				// Don't update selecto as it's already aware of it
				selectedTargets.addSelectedTarget(String(item.id), element as HTMLElement, {
					updateSelecto: false
				});

				explorer.addSelectedItem(item.data);
				if (document.contains(element)) elements.push(element);
			}

			for (const element of e.removed) {
				const item = getGridItem(element);
				if (!item?.data) continue;

				// Remove item from selected targets
				// Don't update selecto as it's already aware of it
				selectedTargets.removeSelectedTarget(String(item.id), { updateSelecto: false });

				// Don't deselect item if element is unmounted by scroll
				if (!document.contains(element)) continue;

				explorer.removeSelectedItem(item.data);
				elements.push(element);
			}

			const dragDirection = {
				x: inputEvent.x === e.rect.left ? 'left' : 'right',
				y: inputEvent.y === e.rect.bottom ? 'down' : 'up'
			} as const;

			const dragStart = {
				x: dragDirection.x === 'right' ? e.rect.left : e.rect.right,
				y: dragDirection.y === 'down' ? e.rect.top : e.rect.bottom
			};

			const dragEnd = {
				x: inputEvent.x,
				y: inputEvent.y
			};

			const dragRect = {
				top: dragDirection.y === 'down' ? dragStart.y : dragEnd.y,
				bottom: dragDirection.y === 'down' ? dragEnd.y : dragStart.y,
				left: dragDirection.x === 'right' ? dragStart.x : dragEnd.x,
				right: dragDirection.x === 'right' ? dragEnd.x : dragStart.x
			};

			// Group elements by column
			const columnItems = elements.reduce(
				(items, element) => {
					const item = getGridItem(element);
					if (!item) return items;

					const columnItem = { item, node: element as HTMLElement };

					let firstItem = items[item.column]?.firstItem ?? columnItem;
					let lastItem = items[item.column]?.lastItem ?? columnItem;

					if (dragDirection.y === 'down') {
						if (item.row < firstItem.item.row) firstItem = columnItem;
						if (item.row > lastItem.item.row) lastItem = columnItem;
					} else {
						if (item.row > firstItem.item.row) firstItem = columnItem;
						if (item.row < lastItem.item.row) lastItem = columnItem;
					}

					items[item.column] = { firstItem, lastItem };

					return items;
				},
				{} as Record<
					number,
					Record<
						'firstItem' | 'lastItem',
						{ item: NonNullable<ReturnType<typeof getGridItem>>; node: HTMLElement }
					>
				>
			);

			const columns = Object.keys(columnItems).map(column => Number(column));

			// Sort columns in drag direction
			columns.sort((a, b) => (dragDirection.x === 'right' ? a - b : b - a));

			// Helper function to check if the element is within the drag area
			const isItemInDragArea = (item: HTMLElement, asis: 'x' | 'y' | 'all' = 'all') => {
				const rect = item.getBoundingClientRect();

				const inX = dragRect.left <= rect.right && dragRect.right >= rect.left;
				const inY = dragRect.top <= rect.bottom && dragRect.bottom >= rect.top;

				return asis === 'all' ? inX && inY : asis === 'x' ? inX : inY;
			};

			const addedColumns = new Set<number>();
			const removedColumns = new Set<number>();

			const addedRows = new Set<number>();
			const removedRows = new Set<number>();

			for (const column of columns) {
				const { firstItem, lastItem } = columnItems[column]!;

				const { row: firstRow } = firstItem.item;
				const { row: lastRow } = lastItem.item;

				const isItemInDrag = isItemInDragArea(lastItem.node);
				const isColumnInDrag = isItemInDragArea(lastItem.node, 'x');
				const isFirstRowInDrag = isItemInDragArea(firstItem.node, 'y');
				const isLastRowInDrag = isItemInDragArea(lastItem.node, 'y');

				const isColumnInDragRange = drag.current
					? dragDirection.x === 'right'
						? column >= drag.current.startColumn && column <= drag.current.endColumn
						: column <= drag.current.startColumn && column >= drag.current.endColumn
					: undefined;

				const isFirstRowInDragRange = drag.current
					? dragDirection.y === 'down'
						? firstRow >= drag.current.startRow && firstRow <= drag.current.endRow
						: firstRow <= drag.current.startRow && firstRow >= drag.current.endRow
					: undefined;

				const isLastRowInDragRange = drag.current
					? dragDirection.y === 'down'
						? lastRow >= drag.current.startRow && lastRow <= drag.current.endRow
						: lastRow <= drag.current.startRow && lastRow >= drag.current.endRow
					: undefined;

				// Remove first row if we drag out of it and it's the starting row of the drag
				if (!isFirstRowInDrag && firstRow === drag.current?.startRow) {
					removedRows.add(firstRow);
				}

				// Remove last row if we drag out of it and it's the ending row of the drag
				if (!isLastRowInDrag && lastRow === drag.current?.endRow) {
					removedRows.add(lastRow);
				}

				// Set new start row if we dragged over a row that's not in the drag range
				if (!isFirstRowInDragRange && isFirstRowInDrag) {
					addedRows.add(firstRow);
				}

				// Set new end row if we dragged over a row that's not in the drag range
				if (!isLastRowInDragRange && isLastRowInDrag) {
					addedRows.add(lastRow);
				}

				// Prevent first row from being removed if it was previously tagged as removable
				// Can happen when the drag event catches multiple columns at once
				if (isFirstRowInDrag && removedRows.has(firstRow)) {
					removedRows.delete(firstRow);
				}

				// Prevent last row from being removed if it was previously tagged as removable
				// Can happen when the drag event catches multiple columns at once
				if (isLastRowInDrag && removedRows.has(lastRow)) {
					removedRows.delete(lastRow);
				}

				// Remove rows if we drag out of the starting column
				if (!isColumnInDrag && column === drag.current?.startColumn) {
					removedRows.add(firstRow);
					removedRows.add(lastRow);
				}

				if (!isColumnInDrag && dragDirection.x === 'left') {
					// Get the item that's closest to grid's end
					const item = dragDirection.y === 'down' ? lastItem : firstItem;

					// Remove row if dragged out of the last grid item
					// from a row that's above it
					if (item.item.index === props.totalCount - 1) {
						removedRows.add(item.item.row);
					}
				}

				// Add column if dragged over and it's not in the drag range
				if (isColumnInDrag && !isColumnInDragRange) {
					addedColumns.add(column);
				}

				// Remove column when dragged out of the column or starting row
				if (!isColumnInDrag || (firstRow === drag.current?.startRow && !isLastRowInDrag)) {
					removedColumns.add(column);
				}

				// Remove columns that are not in the new selected row, when the drag event
				// caches multiple rows at once, and the first one being removed
				if (
					!isFirstRowInDrag &&
					firstRow === props.totalRowCount - 2 &&
					firstItem.item.index + props.totalColumnCount > props.totalCount - 1
				) {
					removedColumns.add(column);
				}

				// Return if first row equals the first/last row of the grid (depending on drag direction)
				// as there's no items to be selected beyond that point
				if (!drag.current && (firstRow === 0 || firstRow === props.totalRowCount - 1)) {
					continue;
				}

				// Return if column is already in drag range
				if (isColumnInDrag && isColumnInDragRange) {
					continue;
				}

				const viewTop = explorerView.ref.current?.getBoundingClientRect().top ?? 0;

				const itemTop = firstItem.item.rect.top + viewTop;
				const itemBottom = firstItem.item.rect.bottom + viewTop;

				const hasEmptySpace =
					dragDirection.y === 'down' ? dragStart.y < itemTop : dragStart.y > itemBottom;

				if (!hasEmptySpace) continue;

				// Get the height of the empty drag space between the start of the drag
				// and the first visible item
				const emptySpaceHeight = Math.abs(
					dragStart.y - (dragDirection.y === 'down' ? itemTop : itemBottom)
				);

				// Check how many items we can fit into the empty space
				let itemsInEmptySpace =
					(emptySpaceHeight - (props.gapY ?? 0)) /
					(props.virtualItemHeight + (props.gapY ?? 0));

				if (itemsInEmptySpace > 1) {
					itemsInEmptySpace = Math.ceil(itemsInEmptySpace);
				} else {
					itemsInEmptySpace = Math.round(itemsInEmptySpace);
				}

				for (let i = 0; i < itemsInEmptySpace; i++) {
					i = dragDirection.y === 'down' ? itemsInEmptySpace - i : i + 1;

					const explorerItemIndex =
						firstItem.item.index +
						(dragDirection.y === 'down' ? -i : i) * props.columnCount;

					const item = props.getItem(explorerItemIndex);
					if (!item?.data) continue;

					// Set start row if not already set
					if (!drag.current && i === itemsInEmptySpace - 1) {
						addedRows.add(item.row);
					}

					if (continueSelection) {
						if (explorer.selectedItems.has(item.data)) {
							explorer.removeSelectedItem(item.data);
						} else {
							explorer.addSelectedItem(item.data);
						}

						continue;
					}

					if (!isItemInDrag) explorer.removeSelectedItem(item.data);
					else explorer.addSelectedItem(item.data);
				}
			}

			const addedColumnsArray = [...addedColumns];
			const removedColumnsArray = [...removedColumns];

			// Sort added rows in drag direction in case we add a row
			// from the empty column drag space
			const addedRowsArray = [...addedRows].sort((a, b) => {
				if (dragDirection.y === 'up') return b - a;
				return a - b;
			});

			const lastAddedColumn = addedColumnsArray[addedColumnsArray.length - 1];
			const lastRemovedColumn = removedColumnsArray[removedColumnsArray.length - 1];
			const lastAddedRow = addedRowsArray[addedRowsArray.length - 1];

			const furthestAddedColumn =
				dragDirection.x === 'right' ? lastAddedColumn : addedColumnsArray[0];

			const furthestRemovedColumn =
				dragDirection.x === 'right' ? lastRemovedColumn : removedColumnsArray[0];

			let startColumn = drag.current?.startColumn;
			let endColumn = drag.current?.endColumn;
			let startRow = drag.current?.startRow;
			let endRow = drag.current?.endRow;

			const isStartRowRemoved = startRow !== undefined && removedRows.has(startRow);
			const isEndRowRemoved = endRow !== undefined && removedRows.has(endRow);

			const isStartColumnRemoved =
				startColumn !== undefined && removedColumns.has(startColumn);

			// Reset drag state if we drag out of the starting point
			// which isn't a selectable item
			if (
				isStartRowRemoved &&
				isStartColumnRemoved &&
				!addedColumns.size &&
				!addedRows.size
			) {
				drag.current = null;
				return;
			}

			// Start column
			if (startColumn !== undefined && dragDirection.x === 'left') {
				if (furthestAddedColumn !== undefined && furthestAddedColumn > startColumn) {
					startColumn = furthestAddedColumn;
				}

				if (
					isEndRowRemoved &&
					furthestRemovedColumn !== undefined &&
					startColumn <= furthestRemovedColumn
				) {
					startColumn = startColumn - removedColumns.size;
				}
			} else if (startColumn === undefined || isStartColumnRemoved) {
				startColumn = addedColumnsArray[0];
			}

			// End column
			if (lastAddedColumn !== undefined) {
				const isLastColumnFurther = endColumn
					? dragDirection.x === 'right'
						? lastAddedColumn > endColumn
						: lastAddedColumn < endColumn
					: undefined;

				if (isLastColumnFurther === undefined || isLastColumnFurther) {
					endColumn = lastAddedColumn;
				}
			} else if (endColumn !== undefined) {
				const offset = removedColumnsArray.filter(column => column <= endColumn!).length;
				endColumn += dragDirection.x === 'right' ? -[offset] : offset;
			}

			// Start row
			if (startRow === undefined || isStartRowRemoved) {
				startRow = addedRowsArray[0] ?? endRow;
			} else if (lastAddedRow !== undefined) {
				const isLastRowAboveStartRow = dragDirection.y === 'up' && lastAddedRow > startRow;
				startRow = isLastRowAboveStartRow ? lastAddedRow : startRow;
			}

			// End row
			if (lastAddedRow !== undefined) {
				const isLastRowFurther = endRow
					? dragDirection.y === 'down'
						? lastAddedRow > endRow
						: lastAddedRow < endRow
					: undefined;

				if (isLastRowFurther === undefined || isLastRowFurther) {
					endRow = lastAddedRow;
				}
			} else if (removedRows.size !== 0 && endRow !== undefined) {
				const offset = removedRows.size;
				const newEndRow = endRow + (dragDirection.y === 'down' ? -[offset] : offset);
				endRow = removedRows.has(newEndRow) ? startRow : newEndRow;
			}

			if (
				startColumn !== undefined &&
				endColumn !== undefined &&
				startRow !== undefined &&
				endRow !== undefined
			) {
				drag.current = { startColumn, endColumn, startRow, endRow };
			}
		}
	}

	return (
		<DragSelectContext.Provider value={{ selecto, drag, ...selectedTargets }}>
			<Selecto
				ref={selecto}
				dragContainer={explorerView.ref.current}
				boundContainer={{
					element: explorerView.ref.current ?? false,
					top: false,
					bottom: false
				}}
				//Prevent mouse side-buttons from drag
				dragCondition={e => {
					return e.inputEvent.buttons === 1;
				}}
				scrollOptions={{
					container: { current: explorer.scrollRef.current },
					throttleTime: isChrome ? 30 : 10000
				}}
				selectableTargets={[`[${SELECTABLE_DATA_ATTRIBUTE}]`]}
				toggleContinueSelect={[['shift'], [isWindows ? 'ctrl' : 'meta']]}
				hitRate={0}
				onDrag={handleDrag}
				onDragStart={handleDragStart}
				onDragEnd={handleDragEnd}
				onScroll={handleScroll}
				onSelect={handleSelect}
			/>

			{children}
		</DragSelectContext.Provider>
	);
};
