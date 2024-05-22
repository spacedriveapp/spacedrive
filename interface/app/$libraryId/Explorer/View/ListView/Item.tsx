import { flexRender, type Cell } from '@tanstack/react-table';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { getItemFilePath, useSelector, type ExplorerItem } from '@sd/client';

import { TABLE_PADDING_X } from '.';
import { useExplorerContext } from '../../Context';
import { ExplorerDraggable } from '../../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../../ExplorerDroppable';
import { explorerStore } from '../../store';
import { ViewItem } from '../ViewItem';
import { useTableContext } from './context';
import { LIST_VIEW_TEXT_SIZES } from './useTable';

interface Props {
	data: ExplorerItem;
	selected: boolean;
	cells: Cell<ExplorerItem, unknown>[];
}

export const ListViewItem = memo(({ data, selected, cells }: Props) => {
	const filePath = getItemFilePath(data);

	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming && selected);

	return (
		<ViewItem
			data={data}
			className="flex"
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
					draggable={{ data, disabled: isRenaming }}
					className={clsx('flex items-center', filePath?.hidden && 'opacity-50')}
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
