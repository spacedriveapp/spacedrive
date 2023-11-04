import { useDraggable, UseDraggableArguments } from '@dnd-kit/core';
import { CSSProperties, useCallback } from 'react';
import { ExplorerItem } from '@sd/client';

import { getExplorerStore } from '../store';
import { uniqueId } from '../util';

interface Props extends Omit<UseDraggableArguments, 'id'> {
	data: ExplorerItem;
}

export const useExplorerDraggable = (props: Props) => {
	const { setNodeRef, ...draggable } = useDraggable({
		...props,
		id: uniqueId(props.data)
	});

	const onMouseDown = useCallback(() => (getExplorerStore().drag = { type: 'touched' }), []);

	const onMouseLeave = useCallback(() => {
		if (getExplorerStore().drag?.type !== 'dragging') getExplorerStore().drag = null;
	}, []);

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
