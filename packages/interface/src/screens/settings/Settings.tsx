import { Suspense } from 'react';
import { Outlet } from 'react-router';

import { SettingsSidebar } from '../../components/settings/SettingsSidebar';

export default function SettingsScreen() {
	return (
		<div className="flex flex-row w-full app-bg">
			<SettingsSidebar />
			<div className="w-full">
				<div data-tauri-drag-region className="w-full h-7" />
				<div className="flex flex-grow-0 w-full h-full max-h-screen custom-scroll page-scroll">
					<div className="flex flex-grow px-12 pb-5">
						<Suspense>
							<Outlet />
						</Suspense>
						<div className="block h-20" />
					</div>
				</div>
			</div>
		</div>
	);
}
