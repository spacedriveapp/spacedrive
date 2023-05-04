import { Outlet } from 'react-router';
import { TOP_BAR_HEIGHT } from './TopBar';

export const Component = () => {
	return (
		<div
			className="custom-scroll page-scroll app-background flex h-screen w-full flex-col"
			style={{ paddingTop: TOP_BAR_HEIGHT }}
		>
			<div className="flex h-screen w-full flex-col p-5">
				<Outlet />
			</div>
		</div>
	);
};
