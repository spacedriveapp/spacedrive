import clsx from 'clsx';
import { PropsWithChildren, RefObject, createContext, useContext, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Outlet } from 'react-router';
import DragRegion from '~/components/DragRegion';
import TopBar from './Explorer/TopBar';

const PageLayoutContext = createContext<{ ref: RefObject<HTMLDivElement> } | null>(null);

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);

	return (
		<PageLayoutContext.Provider value={{ ref }}>
			<TopBar />
			<div
				className={clsx(
					'custom-scrol page-scroll app-background flex h-screen w-full flex-col pt-10'
				)}
			>
				<DragRegion ref={ref} />
				<div className="flex flex-col w-full h-screen p-5 pt-0">
					<Outlet />
				</div>
			</div>
		</PageLayoutContext.Provider>
	);
};

export const DragChildren = ({ children }: PropsWithChildren) => {
	const ctx = useContext(PageLayoutContext);

	if (!ctx) throw new Error('Missing PageLayoutContext');

	const target = ctx.ref.current;

	if (!target) return null;

	return createPortal(children, target);
};
