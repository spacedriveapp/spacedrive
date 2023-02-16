import { Suspense } from 'react';
import { Outlet } from 'react-router';
import { SettingsSidebar } from '~/components/settings/SettingsSidebar';

export default function SettingsScreen() {
	return (
		<div className="app-background flex w-full flex-row">
			<SettingsSidebar />
			<div className="w-full">
				<Suspense>
					<Outlet />
				</Suspense>
			</div>
		</div>
	);
}
