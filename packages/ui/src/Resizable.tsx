'use client';

import clsx from 'clsx';
import {
	createContext,
	HTMLAttributes,
	PropsWithChildren,
	useContext,
	useEffect,
	useRef
} from 'react';
import {
	Resizable as ResizableType,
	useResizable,
	UseResizableProps
} from 'react-resizable-layout';

type ResizableContextProps = ResizableType & { size: number; collapsed: boolean };

const ResizableContext = createContext<ResizableContextProps | null>(null);

const useResizableContext = () => {
	const context = useContext(ResizableContext);

	if (!context) throw new Error('ResizableContext.Provider not found!');

	return context;
};

interface ResizableProps extends Omit<PropsWithChildren<UseResizableProps>, 'axis'> {
	axis?: UseResizableProps['axis'];
	collapsed?: boolean;
	onCollapseChange?: (val: boolean) => void;
}

const Resizable = ({ axis = 'x', ...props }: ResizableProps) => {
	const resizable = useResizable({ axis, ...props });

	const minSizeClientX = useRef<number | null>(null);

	useEffect(() => {
		if (!props.onCollapseChange || !resizable.isDragging || !props.min) return;

		const handleMouseMove = (e: MouseEvent) => {
			if (minSizeClientX.current === null) {
				if (props.min === resizable.position && !props.collapsed) {
					minSizeClientX.current = e.clientX;
				}
				return;
			}

			const half = minSizeClientX.current / 2;

			if (e.clientX < half && !props.collapsed) props.onCollapseChange!(true);
			else if (e.clientX > half && props.collapsed) props.onCollapseChange!(false);
		};

		document.addEventListener('mousemove', handleMouseMove);
		return () => document.removeEventListener('mousemove', handleMouseMove);
	}, [
		props.min,
		props.collapsed,
		props.onCollapseChange,
		resizable.isDragging,
		resizable.position
	]);

	useEffect(() => {
		if (!resizable.isDragging) {
			minSizeClientX.current = null;
			document.body.style.cursor = '';
		} else {
			const cursor = axis === 'x' ? 'col-resize' : 'row-resize';
			document.body.style.setProperty('cursor', cursor, 'important');
		}
	}, [resizable.isDragging, axis]);

	return (
		<ResizableContext.Provider
			value={{
				...resizable,
				size: props.collapsed ? 0 : resizable.position,
				collapsed: !!props.collapsed
			}}
		>
			{props.children}
		</ResizableContext.Provider>
	);
};

const ResizablePanel = (props: HTMLAttributes<HTMLDivElement>) => {
	const resizable = useResizableContext();
	return <div style={{ width: resizable.size }} {...props} />;
};

const ResizableHandle = ({ className, ...props }: HTMLAttributes<HTMLDivElement>) => {
	const resizable = useResizableContext();

	return (
		<div
			className={clsx(
				'w-2',
				'aria-[orientation=horizontal]:cursor-row-resize aria-[orientation=vertical]:cursor-col-resize',
				'after:absolute after:inset-y-0 after:left-0.5 after:w-0.5 after:bg-accent after:opacity-0 after:transition-opacity hover:after:opacity-100',
				resizable.isDragging && 'after:opacity-100',
				className
			)}
			{...props}
			{...resizable.separatorProps}
		/>
	);
};

export { Resizable, ResizableHandle, ResizablePanel, useResizableContext };
