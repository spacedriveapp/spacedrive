import clsx from 'clsx';
import { Outlet } from 'react-router';
import TopBar, { TOP_BAR_HEIGHT } from './TopBar';

export const Component = () => {
	return (
		<>
			<TopBar />
			<div
				className={clsx(
					'custom-scrol page-scroll app-background flex h-screen w-full flex-col'
				)}
				style={{
					paddingTop: TOP_BAR_HEIGHT
				}}
			>
				<div className="flex h-screen w-full flex-col p-5">
					<Outlet />
				</div>
			</div>
		</>
	);
};
