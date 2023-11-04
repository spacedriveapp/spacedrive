import { type Row } from '@tanstack/react-table';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../../Context';
import { useTableContext } from './context';
import { ListViewItem } from './Item';

interface RowProps {
	row: Row<ExplorerItem>;
	previousRow?: Row<ExplorerItem>;
	nextRow?: Row<ExplorerItem>;
}

export const TableRow = memo((props: RowProps) => {
	const explorer = useExplorerContext();
	const table = useTableContext();

	const selected = useMemo(() => {
		return explorer.selectedItems.has(props.row.original);
	}, [explorer.selectedItems, props.row.original]);

	const selectedPrior = useMemo(() => {
		if (!props.previousRow) return;
		return explorer.selectedItems.has(props.previousRow.original);
	}, [explorer.selectedItems, props.previousRow]);

	const selectedNext = useMemo(() => {
		if (!props.nextRow) return;
		return explorer.selectedItems.has(props.nextRow.original);
	}, [explorer.selectedItems, props.nextRow]);

	return (
		<>
			<div
				className={clsx(
					'absolute inset-0 rounded-md border',
					props.row.index % 2 === 0 && 'bg-app-darkBox',
					selected ? 'border-accent !bg-accent/10' : 'border-transparent',
					selected && selectedPrior && 'rounded-t-none border-t-0 border-t-transparent',
					selected && selectedNext && 'rounded-b-none border-b-0 border-b-transparent'
				)}
				style={{ left: table.padding.left, right: table.padding.right }}
			>
				{selectedPrior && <div className="absolute inset-x-3 top-0 h-px bg-accent/10" />}
			</div>

			<ListViewItem row={props.row} selected={selected} />
		</>
	);
});
