import { PropsWithChildren, ReactNode, Suspense } from 'react';
import { Outlet } from 'react-router';
import { useRouteTitle } from '~/hooks';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { useWindowState } from '~/hooks/useWindowState';

import DragRegion from '../../../components/DragRegion';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import Sidebar from './Sidebar';

export const Component = () => {
	const os = useOperatingSystem();
	const windowState = useWindowState();

	useRouteTitle('Settings');

	return (
		<div className={`flex w-full flex-row bg-app ${os === 'windows' && 'mt-6'}`}>
			{os === 'windows' && <TopBarPortal right={<TopBarOptions />} />}
			<Sidebar />
			<div className="relative w-full">
				<Suspense>
					{os === 'macOS' && !windowState.isFullScreen && (
						<DragRegion className="absolute inset-x-0 top-0 z-50 h-8" />
					)}
					<Outlet />
				</Suspense>
			</div>
		</div>
	);
};

interface HeaderProps extends PropsWithChildren {
	title: string;
	description: string | ReactNode;
	rightArea?: ReactNode;
}

export const Heading = (props: HeaderProps) => {
	return (
		<div className="mb-3 flex">
			{props.children}
			<div className="grow">
				<h1 className="text-2xl font-bold">{props.title}</h1>
				<p className="mt-1 text-sm text-gray-400">{props.description}</p>
			</div>
			{props.rightArea}
			<hr className="mt-4 border-gray-550" />
		</div>
	);
};
