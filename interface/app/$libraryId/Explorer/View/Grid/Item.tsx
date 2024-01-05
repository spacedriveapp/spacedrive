import { HTMLAttributes, useEffect, useMemo } from 'react';
import { type ExplorerItem } from '@sd/client';

import { RenderItem } from '.';
import { useExplorerContext } from '../../Context';
import { explorerStore, isCut } from '../../store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { useGridContext } from './context';

interface Props extends Omit<HTMLAttributes<HTMLDivElement>, 'children'> {
	index: number;
	item: ExplorerItem;
	children: RenderItem;
}

export const GridItem = ({ children, item, ...props }: Props) => {
	const grid = useGridContext();
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const itemId = useMemo(() => uniqueId(item), [item]);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(item),
		[explorer.selectedItems, item]
	);

	const cut = useMemo(
		() => isCut(item, explorerStore.cutCopyState),
		[explorerStore.cutCopyState, item]
	);

	useEffect(() => {
		if (!grid.selecto?.current || !grid.selectoUnselected.current.has(itemId)) return;

		if (!selected) {
			grid.selectoUnselected.current.delete(itemId);
			return;
		}

		const element = grid.getElementById(itemId);

		if (!element) return;

		grid.selectoUnselected.current.delete(itemId);
		grid.selecto.current.setSelectedTargets([
			...grid.selecto.current.getSelectedTargets(),
			element as HTMLElement
		]);

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	useEffect(() => {
		if (!grid.selecto) return;

		return () => {
			const element = grid.getElementById(itemId);
			if (selected && !element) grid.selectoUnselected.current.add(itemId);
		};

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [selected]);

	return (
		<div
			{...props}
			className="h-full w-full"
			data-selectable=""
			data-selectable-index={props.index}
			data-selectable-id={itemId}
			onContextMenu={(e) => {
				if (explorerView.selectable && !explorer.selectedItems.has(item)) {
					explorer.resetSelectedItems([item]);
					grid.selecto?.current?.setSelectedTargets([e.currentTarget]);
				}
			}}
		>
			{children({ item: item, selected, cut })}
		</div>
	);
};
