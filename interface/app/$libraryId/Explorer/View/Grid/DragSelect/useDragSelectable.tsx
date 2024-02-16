import { useEffect } from 'react';

import { useDragSelectContext } from './context';
import {
	getElementByIndex,
	SELECTABLE_DATA_ATTRIBUTE,
	SELECTABLE_INDEX_DATA_ATTRIBUTE
} from './util';

export interface UseDragSelectableProps {
	index: number;
	id: string;
	selected: boolean;
}

export const useDragSelectable = (props: UseDragSelectableProps) => {
	const dragSelect = useDragSelectContext();

	const attributes = {
		[SELECTABLE_DATA_ATTRIBUTE]: '',
		[SELECTABLE_INDEX_DATA_ATTRIBUTE]: props.index
		// [SELECTABLE_ID_DATA_ATTRIBUTE]: props.id
	};

	useEffect(() => {
		const selecto = dragSelect.selecto.current;
		if (!selecto) return;

		const node = getElementByIndex(props.index);
		if (!node) return;

		const target = dragSelect.selectedTargets.current.get(props.id);

		if (!target && props.selected) dragSelect.addSelectedTarget(props.id, node as HTMLElement);
		else if (target) {
			if (!props.selected) dragSelect.removeSelectedTarget(props.id);
			else if (!document.contains(target)) {
				dragSelect.addSelectedTarget(props.id, node as HTMLElement);
			}
		}

		return () => {
			if (props.selected) dragSelect.removeSelectedTarget(props.id);
		};

		// Passing the dragSelect object will just cause unnecessary re-runs
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [
		props.id,
		props.selected,
		dragSelect.selecto,
		dragSelect.selectedTargets,
		dragSelect.addSelectedTarget,
		dragSelect.removeSelectedTarget
	]);

	return { attributes };
};
