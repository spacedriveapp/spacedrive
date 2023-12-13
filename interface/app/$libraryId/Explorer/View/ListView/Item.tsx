import { flexRender, type Cell } from '@tanstack/react-table';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { getItemFilePath, type ExplorerItem } from '@sd/client';

import { TABLE_PADDING_X } from '.';
import { ExplorerDraggable } from '../../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../../ExplorerDroppable';
import { ViewItem } from '../ViewItem';
import { useTableContext } from './context';

interface Props {
	data: ExplorerItem;
	selected: boolean;
	cells: Cell<ExplorerItem, unknown>[];
}

export const ListViewItem = memo(({ data, selected, cells }: Props) => {
	const filePath = getItemFilePath(data);

	return (
		<ViewItem
			data={data}
			className="flex h-full"
			style={{ paddingLeft: TABLE_PADDING_X, paddingRight: TABLE_PADDING_X }}
		>
			<ExplorerDroppable
				className="relative"
				droppable={{
					data: { type: 'explorer-item', data },
					disabled: (!filePath?.is_dir && data.type !== 'Location') || selected
				}}
			>
				<DroppableOverlay />
				<ExplorerDraggable
					draggable={{ data }}
					className={clsx('flex h-full items-center', filePath?.hidden && 'opacity-50')}
				>
					{cells.map((cell) => (
						<Cell key={cell.id} cell={cell} selected={selected} />
					))}
				</ExplorerDraggable>
			</ExplorerDroppable>
		</ViewItem>
	);
});

const DroppableOverlay = () => {
	const { isDroppable } = useExplorerDroppableContext();
	if (!isDroppable) return null;

	return <div className="absolute inset-0 rounded-md bg-accent/25" />;
};

const Cell = ({ cell, selected }: { cell: Cell<ExplorerItem, unknown>; selected: boolean }) => {
	useTableContext(); // Force re-render for column sizing

	return <InnerCell cell={cell} size={cell.column.getSize()} selected={selected} />;
};

const InnerCell = memo(
	(props: { cell: Cell<ExplorerItem, unknown>; size: number; selected: boolean }) => {
		const value = useMemo(() => props.cell.getValue(), [props.cell]);

		return (
			<div
				key={props.cell.id}
				className={clsx(
					'table-cell px-4 text-xs text-ink-dull',
					props.cell.column.id !== 'name' && 'truncate',
					props.cell.column.columnDef.meta?.className
				)}
				style={{ width: props.size }}
			>
				{value
					? `${value}`
					: flexRender(props.cell.column.columnDef.cell, {
							...props.cell.getContext(),
							selected: props.selected
					  })}
			</div>
		);
	}
);
