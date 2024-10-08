import clsx from 'clsx';
import React, { memo } from 'react';
import { getItemFilePath, useSelector, type ExplorerItem } from '@sd/client';

import { ExplorerDraggable } from '../ExplorerDraggable';
import { ExplorerDroppable, useExplorerDroppableContext } from '../ExplorerDroppable';
import { explorerStore } from '../store';
import { ViewItem } from './ViewItem';

interface Props {
	data: ExplorerItem;
	selected: boolean;
	cells: React.ReactNode[];
}

export const RowViewItem = memo(({ data, selected, cells }: Props) => {
	const filePath = getItemFilePath(data);

	const isRenaming = useSelector(explorerStore, (s) => s.isRenaming && selected);

	return (
		<ViewItem data={data} className="flex" style={{ paddingLeft: 16, paddingRight: 16 }}>
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
					{cells}
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
