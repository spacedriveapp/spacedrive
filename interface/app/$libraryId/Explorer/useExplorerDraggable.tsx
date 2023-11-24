import { useDraggable, UseDraggableArguments } from '@dnd-kit/core';
import { CSSProperties } from 'react';
import { ExplorerItem } from '@sd/client';

import { getExplorerStore } from './store';
import { uniqueId } from './util';

interface Props extends Omit<UseDraggableArguments, 'id'> {
	data: ExplorerItem;
}

const draggableTypes: ExplorerItem['type'][] = ['Path', 'NonIndexedPath', 'Object'];

export const useExplorerDraggable = (props: Props) => {
	const disabled = props.disabled || !draggableTypes.includes(props.data.type);

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
