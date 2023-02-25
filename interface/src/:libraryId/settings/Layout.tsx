import { PropsWithChildren, ReactNode, Suspense } from 'react';
import { Outlet } from 'react-router';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import Sidebar from './Sidebar';

export default () => {
	const os = useOperatingSystem();

	return (
		<div className="app-background flex w-full flex-row">
			<Sidebar />
			<div className="w-full">
				{os !== 'browser' ? (
					<div data-tauri-drag-region className="h-3 w-full" />
				) : (
					<div className="h-5" />
				)}
				<Suspense>
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

export const Header = (props: HeaderProps) => {
	return (
		<div className="mb-3 flex">
			{props.children}
			<div className="grow">
				<h1 className="text-2xl font-bold">{props.title}</h1>
				<p className="mt-1 text-sm text-gray-400">{props.description}</p>
			</div>
			{props.rightArea}
			<hr className="border-gray-550 mt-4" />
		</div>
	);
};
