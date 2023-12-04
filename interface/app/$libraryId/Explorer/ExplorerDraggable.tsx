import { HTMLAttributes } from 'react';

import { useExplorerDraggable, UseExplorerDraggableProps } from './useExplorerDraggable';

/**
 * Wrapper for explorer draggable items until dnd-kit solvers their re-rendering issues
 * https://github.com/clauderic/dnd-kit/issues/1194#issuecomment-1696704815
 */
export const ExplorerDraggable = ({
	draggable,
	...props
}: Omit<HTMLAttributes<HTMLDivElement>, 'draggable'> & {
	draggable: UseExplorerDraggableProps;
}) => {
	const { attributes, listeners, style, setDraggableRef } = useExplorerDraggable(draggable);

	return (
		<div
			{...props}
			ref={setDraggableRef}
			style={{ ...props.style, ...style }}
			{...attributes}
			{...listeners}
		>
			{props.children}
		</div>
	);
};
