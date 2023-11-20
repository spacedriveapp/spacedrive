import { useEffect, useMemo } from 'react';
import { type ExplorerItem } from '@sd/client';

import { RenderItem } from '.';
import { useExplorerContext } from '../../Context';
import { isCut } from '../../store';
import { uniqueId } from '../../util';
import { useExplorerViewContext } from '../../ViewContext';
import { useGridContext } from './context';

interface Props {
	index: number;
	item: ExplorerItem;
	children: RenderItem;
	onMouseDown: (e: React.MouseEvent<HTMLDivElement, MouseEvent>) => void;
	getElementById: (id: string) => Element | null | undefined;
}

export const GridItem = (props: Props) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const grid = useGridContext();

	const itemId = useMemo(() => uniqueId(props.item), [props.item]);

	const cut = useMemo(() => isCut(props.item), [props.item]);

	const selected = useMemo(
		// Even though this checks object equality, it should still be safe since `selectedItems`
		// will be re-calculated before this memo runs.
		() => explorer.selectedItems.has(props.item),
		[explorer.selectedItems, props.item]
	);

	useEffect(() => {
		if (!grid.selecto?.current || !grid.selectoUnselected.current.has(itemId)) return;

		if (!selected) {
			grid.selectoUnselected.current.delete(itemId);
			return;
		}

		const element = props.getElementById(itemId);

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
			const element = props.getElementById(itemId);
			if (selected && !element) grid.selectoUnselected.current.add(itemId);
		};

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [selected]);

	return (
		<div
			className="h-full w-full"
			data-selectable=""
			data-selectable-index={props.index}
			data-selectable-id={itemId}
			onMouseDown={props.onMouseDown}
			onContextMenu={(e) => {
				if (explorerView.selectable && !explorer.selectedItems.has(props.item)) {
					explorer.resetSelectedItems([props.item]);
					grid.selecto?.current?.setSelectedTargets([e.currentTarget]);
				}
			}}
		>
			{props.children({ item: props.item, selected, cut })}
		</div>
	);
};
