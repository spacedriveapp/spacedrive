import { useDraggable, UseDraggableArguments } from '@dnd-kit/core';
import { CSSProperties, HTMLAttributes } from 'react';
import { ExplorerItem } from '@sd/client';

import { explorerStore } from './store';
import { uniqueId } from './util';

export interface UseExplorerDraggableProps extends Omit<UseDraggableArguments, 'id'> {
	data: ExplorerItem;
}

const draggableTypes: ExplorerItem['type'][] = ['Path', 'NonIndexedPath', 'Object'];

export const useExplorerDraggable = (props: UseExplorerDraggableProps) => {
	const disabled = props.disabled || !draggableTypes.includes(props.data.type);

	const { setNodeRef, ...draggable } = useDraggable({
		...props,
		id: uniqueId(props.data),
		disabled: disabled
	});

	const onMouseDown = () => {
		if (!disabled) explorerStore.drag = { type: 'touched' };
	};

	const onMouseLeave = () => {
		if (explorerStore.drag?.type !== 'dragging') explorerStore.drag = null;
	};

	const onMouseUp = () => (explorerStore.drag = null);

	const style = {
		cursor: 'default',
		outline: 'none'
	} satisfies CSSProperties;

	return {
		...draggable,
		setDraggableRef: setNodeRef,
		listeners: {
			...draggable.listeners,
			onMouseDown,
			onMouseLeave,
			onMouseUp
		} satisfies HTMLAttributes<Element>,
		style
	};
};
