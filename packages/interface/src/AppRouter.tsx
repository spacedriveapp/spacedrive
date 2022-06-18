import React, { useEffect } from 'react';
import { Route, Routes, useLocation } from 'react-router-dom';

import { AppLayout } from './AppLayout';
import { NotFound } from './NotFound';
import { ContentScreen } from './screens/Content';
import { DebugScreen } from './screens/Debug';
import { ExplorerScreen } from './screens/Explorer';
import { OverviewScreen } from './screens/Overview';
import { PhotosScreen } from './screens/Photos';
import { RedirectPage } from './screens/Redirect';
import { SettingsRoutes } from './screens/Settings';
import { TagScreen } from './screens/Tag';

export function AppRouter() {
	let location = useLocation();
	let state = location.state as { backgroundLocation?: Location };

	useEffect(() => {
		console.log({ url: location.pathname });
	}, [state]);

	return (
		<>
			<Routes location={state?.backgroundLocation || location}>
				<Route path="/" element={<AppLayout />}>
					<Route index element={<RedirectPage to="/overview" />} />
					<Route path="overview" element={<OverviewScreen />} />
					<Route path="content" element={<ContentScreen />} />
					<Route path="photos" element={<PhotosScreen />} />
					<Route path="debug" element={<DebugScreen />} />
					<Route path="settings/*" element={<SettingsRoutes />} />
					<Route path="explorer/:id" element={<ExplorerScreen />} />
					<Route path="tag/:id" element={<TagScreen />} />
					<Route path="*" element={<NotFound />} />
				</Route>
			</Routes>
			{state?.backgroundLocation && <SettingsRoutes modal />}
		</>
	);
}
