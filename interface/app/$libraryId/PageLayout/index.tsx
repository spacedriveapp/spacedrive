import { useRef } from 'react';
import { Outlet } from 'react-router';
import { TOP_BAR_HEIGHT } from '../TopBar';
import { PageLayoutContext } from './Context';

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);

	return (
		<PageLayoutContext.Provider value={{ ref }}>
			<div
				ref={ref}
				className="custom-scroll topbar-page-scroll app-background flex h-screen w-full flex-1 flex-col"
				style={{ paddingTop: TOP_BAR_HEIGHT }}
			>
				<div className="flex h-screen w-full flex-1 flex-col">
					<Outlet />
				</div>
			</div>
		</PageLayoutContext.Provider>
	);
};
