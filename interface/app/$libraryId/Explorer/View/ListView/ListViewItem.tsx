import { flexRender, type Row } from '@tanstack/react-table';
import clsx from 'clsx';
import { memo } from 'react';
import { getItemFilePath, type ExplorerItem } from '@sd/client';

import { useExplorerDraggable } from '../../useExplorerDraggable';
import { useExplorerDroppable } from '../../useExplorerDroppable';
import { ViewItem } from '../ViewItem';
import { useTableContext } from './context';

interface ListViewItemProps {
	row: Row<ExplorerItem>;
	selected?: boolean;
}

export const ListViewItem = memo(({ row, selected }: ListViewItemProps) => {
	const table = useTableContext();

	const filePath = getItemFilePath(row.original);

	const { isDroppable, navigateClassName, setDroppableRef } = useExplorerDroppable({
		data: { type: 'explorer-item', data: row.original },
		disabled: !filePath?.is_dir || selected
	});

	const { listeners, attributes, style, setDraggableRef } = useExplorerDraggable({
		data: row.original
	});

	return (
		<ViewItem
			ref={setDroppableRef}
			data={row.original}
			className={clsx('relative flex h-full items-center', navigateClassName)}
			style={{ paddingLeft: table.padding.left, paddingRight: table.padding.right }}
		>
			{isDroppable && (
				<div
					className="absolute inset-0 rounded-md bg-accent/25"
					style={{ left: table.padding.left, right: table.padding.right }}
				/>
			)}

			{row.getVisibleCells().map((cell) => (
				<div
					key={cell.id}
					className={clsx(
						'relative flex h-full shrink-0 items-center px-4 text-xs text-ink-dull',
						cell.column.id !== 'name' && 'truncate',
						cell.column.columnDef.meta?.className,
						filePath?.hidden && 'opacity-50'
					)}
					style={{ width: cell.column.getSize(), ...style }}
					ref={setDraggableRef}
					{...attributes}
					{...listeners}
				>
					{flexRender(cell.column.columnDef.cell, cell.getContext())}
				</div>
			))}
		</ViewItem>
	);
});
