import { Suspense } from 'react';
import { Outlet } from 'react-router';

export default function SettingsScreen() {
	return (
		<div className="flex flex-row w-full app-background">
			<div className="w-full">
				<Suspense>
					<Outlet />
				</Suspense>
			</div>
		</div>
	);
}
