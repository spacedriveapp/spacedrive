import { flexRender, type Cell, type Row } from '@tanstack/react-table';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { type ExplorerItem } from '@sd/client';

import { TABLE_PADDING_X } from '.';
import { useExplorerContext } from '../../Context';
import { RowViewItem } from '../RowViewItem';
import { useTableContext } from './context';
import { LIST_VIEW_TEXT_SIZES } from './useTable';

interface Props {
	row: Row<ExplorerItem>;
	previousRow?: Row<ExplorerItem>;
	nextRow?: Row<ExplorerItem>;
}

export const TableRow = ({ row, previousRow, nextRow }: Props) => {
	const explorer = useExplorerContext();

	const selected = useMemo(() => {
		return explorer.selectedItems.has(row.original);
	}, [explorer.selectedItems, row.original]);

	const isPreviousRowSelected = useMemo(() => {
		if (!previousRow) return;
		return explorer.selectedItems.has(previousRow.original);
	}, [explorer.selectedItems, previousRow]);

	const isNextRowSelected = useMemo(() => {
		if (!nextRow) return;
		return explorer.selectedItems.has(nextRow.original);
	}, [explorer.selectedItems, nextRow]);

	const cells = row
		.getVisibleCells()
		.map((cell) => <CellComponent key={cell.id} cell={cell} selected={selected} />);

	return (
		<>
			<div
				className={clsx(
					'absolute inset-0 rounded-md border',
					row.index % 2 === 0 && 'bg-app-darkBox',
					selected ? 'border-accent !bg-accent/10' : 'border-transparent',
					selected && [
						isPreviousRowSelected && 'rounded-t-none border-t-0 border-t-transparent',
						isNextRowSelected && 'rounded-b-none border-b-0 border-b-transparent'
					]
				)}
				style={{ left: TABLE_PADDING_X, right: TABLE_PADDING_X }}
			>
				{isPreviousRowSelected && (
					<div className="absolute inset-x-3 top-0 h-px bg-accent/10" />
				)}
			</div>

			<RowViewItem data={row.original} selected={selected} cells={cells} />
		</>
	);
};

const CellComponent = ({
	cell,
	selected
}: {
	cell: Cell<ExplorerItem, unknown>;
	selected: boolean;
}) => {
	useTableContext(); // Force re-render for column sizing

	const explorer = useExplorerContext();
	const explorerSetting = explorer.useSettingsSnapshot();

	return (
		<div
			className={clsx(
				'table-cell px-4 py-1.5 text-ink-dull',
				cell.column.id !== 'name' && 'truncate',
				cell.column.columnDef.meta?.className
			)}
			style={{
				width: cell.column.getSize(),
				fontSize: LIST_VIEW_TEXT_SIZES[explorerSetting.listViewTextSize]
			}}
		>
			<InnerCell cell={cell} selected={selected} />
		</div>
	);
};

const InnerCell = memo((props: { cell: Cell<ExplorerItem, unknown>; selected: boolean }) => {
	const value = useMemo(() => props.cell.getValue(), [props.cell]);

	if (value !== undefined && value !== null) return `${value}`;

	return flexRender(props.cell.column.columnDef.cell, {
		...props.cell.getContext(),
		selected: props.selected
	});
});
