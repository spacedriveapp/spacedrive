import clsx from 'clsx';
import { PropsWithChildren, RefObject, createContext, useContext, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Outlet } from 'react-router';
import DragRegion from '~/components/DragRegion';
import TopBar from './TopBar';

const PageLayoutContext = createContext<{ ref: RefObject<HTMLDivElement> } | null>(null);

interface TopBarContext {
	topBarChildrenRef: RefObject<HTMLDivElement> | null;
}
export const TopBarContext = createContext<TopBarContext>({
	topBarChildrenRef: { current: null }
});

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);
	const topBarChildrenRef = useRef<HTMLDivElement>(null);

	return (
		<TopBarContext.Provider value={{ topBarChildrenRef }}>
			<PageLayoutContext.Provider value={{ ref }}>
				<TopBar ref={topBarChildrenRef} />
				<div
					className={clsx(
						'custom-scrol page-scroll app-background flex h-screen w-full flex-col pt-10'
					)}
				>
					<DragRegion ref={ref} />
					<div className="flex h-screen w-full flex-col p-5 pt-0">
						<Outlet />
					</div>
				</div>
			</PageLayoutContext.Provider>
		</TopBarContext.Provider>
	);
};
export const DragChildren = ({ children }: PropsWithChildren) => {
	const ctx = useContext(PageLayoutContext);
	if (!ctx) throw new Error('Missing PageLayoutContext');

	const target = ctx.ref.current;

	if (!target) return null;

	return createPortal(children, target);
};
