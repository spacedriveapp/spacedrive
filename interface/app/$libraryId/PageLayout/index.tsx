import clsx from 'clsx';
import { useRef } from 'react';
import { Outlet } from 'react-router';

import { useShowControls } from '../../../hooks';
import { useTopBarContext } from '../TopBar/Context';
import { PageLayoutContext } from './Context';

export const Component = () => {
	const ref = useRef<HTMLDivElement>(null);
	const transparentBg = useShowControls().transparentBg;

	const topBar = useTopBarContext();

	return (
		<div
			ref={ref}
			className={clsx(
				'custom-scroll topbar-page-scroll flex h-screen w-full flex-col',
				transparentBg ? 'bg-app/50' : 'bg-app'
			)}
			style={{ paddingTop: topBar.topBarHeight }}
		>
			<PageLayoutContext.Provider value={{ ref }}>
				<div className="flex w-full flex-1 flex-col">
					<Outlet />
				</div>
			</PageLayoutContext.Provider>
		</div>
	);
};
