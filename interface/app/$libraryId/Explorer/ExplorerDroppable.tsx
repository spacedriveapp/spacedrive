import clsx from 'clsx';
import { createContext, HTMLAttributes, useContext, useMemo } from 'react';

import { useExplorerDroppable, UseExplorerDroppableProps } from './useExplorerDroppable';

const ExplorerDroppableContext = createContext<{ isDroppable: boolean } | null>(null);

export const useExplorerDroppableContext = () => {
	const ctx = useContext(ExplorerDroppableContext);

	if (ctx === null) throw new Error('ExplorerDroppableContext.Provider not found!');

	return ctx;
};

/**
 * Wrapper for explorer droppable items until dnd-kit solvers their re-rendering issues
 * https://github.com/clauderic/dnd-kit/issues/1194#issuecomment-1696704815
 */
export const ExplorerDroppable = ({
	droppable,
	children,
	...props
}: HTMLAttributes<HTMLDivElement> & { droppable: UseExplorerDroppableProps }) => {
	const { isDroppable, className, setDroppableRef } = useExplorerDroppable(droppable);

	const context = useMemo(() => ({ isDroppable }), [isDroppable]);

	return (
		<ExplorerDroppableContext.Provider value={context}>
			<div {...props} ref={setDroppableRef} className={clsx(props.className, className)}>
				{children}
			</div>
		</ExplorerDroppableContext.Provider>
	);
};
