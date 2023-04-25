import { Outlet } from 'react-router';
import { TOP_BAR_HEIGHT } from './TopBar';

export const Component = () => {
	return (
		<PageLayoutContext.Provider value={{ ref }}>
			<div
				className={clsx(
					'custom-scroll page-scroll app-background relative flex h-screen w-full flex-col'
				)}
			>
				<DragRegion ref={ref} />
				<div className="flex h-full w-full flex-col p-5 pt-0">
					<Outlet />
				</div>
			</div>
		</div>
	);
};
