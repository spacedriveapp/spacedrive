import { type Row } from '@tanstack/react-table';
import clsx from 'clsx';
import { useMemo } from 'react';
import { type ExplorerItem } from '@sd/client';

import { TABLE_PADDING_X } from '.';
import { useExplorerContext } from '../../Context';
import { ListViewItem } from './Item';

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

	const cells = row.getVisibleCells();

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

			<ListViewItem data={row.original} selected={selected} cells={cells} />
		</>
	);
};
