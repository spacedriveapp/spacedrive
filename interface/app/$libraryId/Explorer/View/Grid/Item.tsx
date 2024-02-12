import { HTMLAttributes, ReactNode, useMemo } from 'react';
import { useSelector, type ExplorerItem } from '@sd/client';

import { useExplorerContext } from '../../Context';
import { explorerStore, isCut } from '../../store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { useDragSelectContext } from './DragSelect/context';
import { useDragSelectable } from './DragSelect/useDragSelectable';

interface Props extends Omit<HTMLAttributes<HTMLDivElement>, 'children'> {
	index: number;
	item: ExplorerItem;
	children: (state: { selected: boolean; cut: boolean }) => ReactNode;
}

export const GridItem = ({ children, item, index, ...props }: Props) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const dragSelect = useDragSelectContext();

	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);

	const cut = useMemo(() => isCut(item, cutCopyState), [cutCopyState, item]);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(item),
		[explorer.selectedItems, item]
	);

	const { attributes } = useDragSelectable({ index, id: uniqueId(item), selected });

	return (
		<div
			{...props}
			{...attributes}
			className="h-full w-full"
			// Prevent explorer view onMouseDown event from
			// being executed and resetting the selection
			onMouseDown={(e) => e.stopPropagation()}
			onContextMenu={(e) => {
				if (!explorerView.selectable || explorer.selectedItems.has(item)) return;
				explorer.resetSelectedItems([item]);
				dragSelect.resetSelectedTargets([{ id: uniqueId(item), node: e.currentTarget }]);
			}}
		>
			{children({ selected, cut })}
		</div>
	);
};
