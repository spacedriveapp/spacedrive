import clsx from 'clsx';
import { RefObject, createContext, useContext, useRef } from 'react';
import { Outlet } from 'react-router';
import { TOP_BAR_HEIGHT } from './TopBar';

const PageContext = createContext<{ ref: RefObject<HTMLDivElement> } | undefined>(undefined);
export const usePageLayout = () => useContext(PageContext);

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);
	const transparentBg = window.location.search.includes('transparentBg');

	return (
		<div
			ref={ref}
			className={clsx(
				'custom-scroll topbar-page-scroll flex h-screen w-full flex-col',
				transparentBg ? 'bg-app/50' : 'bg-app'
			)}
			style={{ paddingTop: TOP_BAR_HEIGHT }}
		>
			<PageContext.Provider value={{ ref }}>
				<div className="flex h-screen w-full flex-col">
					<Outlet />
				</div>
			</PageContext.Provider>
		</div>
	);
};
