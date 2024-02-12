import { HTMLAttributes, PropsWithChildren } from 'react';

import { useDragSelectable, UseDragSelectableProps } from './useDragSelectable';

interface DragSelectableProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	selectable: UseDragSelectableProps;
}

export const DragSelectable = ({ children, selectable, ...props }: DragSelectableProps) => {
	const { attributes } = useDragSelectable(selectable);

	return (
		<div {...props} {...attributes}>
			{children}
		</div>
	);
};
