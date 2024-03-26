import { HTMLAttributes, ReactNode, useMemo } from 'react';
import { useNavigate } from 'react-router';
import { useSelector, type ExplorerItem } from '@sd/client';
import { useOperatingSystem } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';

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
	const { currentIndex, maxIndex } = useRoutingContext();
	const os = useOperatingSystem();
	const navigate = useNavigate();

	const dragSelect = useDragSelectContext();

	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);

	const cut = useMemo(() => isCut(item, cutCopyState), [cutCopyState, item]);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(item),
		[explorer.selectedItems, item]
	);

	const canGoBack = currentIndex !== 0;
	const canGoForward = currentIndex !== maxIndex;

	const { attributes } = useDragSelectable({ index, id: uniqueId(item), selected });

	return (
		<div
			{...props}
			{...attributes}
			className="size-full"
			// Prevent explorer view onMouseDown event from
			// being executed and resetting the selection
			onMouseDown={(e) => {
				e.stopPropagation();
				if (os === 'browser') return;
				if (e.buttons === 8 || e.buttons === 3) {
					if (!canGoBack) return;
					navigate(-1);
				} else if (e.buttons === 16 || e.buttons === 4) {
					if (!canGoForward) return;
					navigate(1);
				}
			}}
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
