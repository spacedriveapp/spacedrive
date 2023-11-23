import { useDraggable, UseDraggableArguments } from '@dnd-kit/core';
import { CSSProperties } from 'react';
import { ExplorerItem } from '@sd/client';

import { useExplorerContext } from './Context';
import { getExplorerStore } from './store';
import { ExplorerParent } from './useExplorer';
import { uniqueId } from './util';

interface Props extends Omit<UseDraggableArguments, 'id'> {
	data: ExplorerItem;
}

const allow: ExplorerParent['type'][] = ['Location', 'Ephemeral'];

export const useExplorerDraggable = (props: Props) => {
	const explorer = useExplorerContext();
	const explorerParentType = explorer.parent?.type;

	const disabled = props.disabled || !explorerParentType || !allow.includes(explorerParentType);

	const { setNodeRef, ...draggable } = useDraggable({
		...props,
		id: uniqueId(props.data),
		disabled: disabled
	});

	const onMouseDown = () => {
		if (!disabled) getExplorerStore().drag = { type: 'touched' };
	};

	const onMouseLeave = () => {
		const explorerStore = getExplorerStore();
		if (explorerStore.drag?.type !== 'dragging') explorerStore.drag = null;
	};

	const style = {
		cursor: 'default',
		outline: 'none'
	} satisfies CSSProperties;

	return {
		...draggable,
		setDraggableRef: setNodeRef,
		listeners: { ...draggable.listeners, onMouseDown, onMouseLeave },
		style
	};
};
