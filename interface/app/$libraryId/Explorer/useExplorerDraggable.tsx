import { useDraggable, UseDraggableArguments } from '@dnd-kit/core';
import { CSSProperties, HTMLAttributes, useCallback, useMemo } from 'react';

import { ExplorerItem } from '@sd/client';

import { explorerStore } from './store';
import { uniqueId } from './util';

export interface UseExplorerDraggableProps extends Omit<UseDraggableArguments, 'id'> {
	data: ExplorerItem;
}

const draggableTypes: ExplorerItem['type'][] = ['Path', 'NonIndexedPath', 'Object'];

const DRAGGABLE_STYLE = {
	cursor: 'default',
	outline: 'none'
} satisfies CSSProperties;

/**
 * This hook is used to make an explorer item draggable.
 *
 * .. WARNING::
 *    This hook is used inside every thumbnail in the explorer.
 * 	  Be careful with the performance of the code, make sure to always memoize any objects or functions to avoid unnecessary re-renders.
 *
 * @param props Draggable properties
 * @returns Draggable properties with additional explorer-specific properties
 */
export const useExplorerDraggable = (props: UseExplorerDraggableProps) => {
	const disabled = useMemo(
		() => props.disabled || !draggableTypes.includes(props.data.type),
		[props.disabled, props.data.type]
	);

	const { setNodeRef, ...draggable } = useDraggable({
		...props,
		id: uniqueId(props.data),
		disabled: disabled
	});

	const onMouseDown = useCallback(() => {
		if (!disabled) explorerStore.drag = { type: 'touched' };
	}, [disabled]);

	const onMouseLeave = useCallback(() => {
		if (explorerStore.drag?.type !== 'dragging') explorerStore.drag = null;
	}, []);

	const onMouseUp = useCallback(() => {
		explorerStore.drag = null;
	}, []);

	return {
		...draggable,
		setDraggableRef: setNodeRef,
		listeners: useMemo(
			() => ({
				...draggable.listeners,
				onMouseDown,
				onMouseLeave,
				onMouseUp
			}),
			[draggable.listeners, onMouseDown, onMouseLeave, onMouseUp]
		) satisfies HTMLAttributes<Element>,
		style: DRAGGABLE_STYLE
	};
};
