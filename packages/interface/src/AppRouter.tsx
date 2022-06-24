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
import { TagScreen } from './screens/Tag';
import { SettingsScreen } from './screens/settings/NodeSettings';
import AppearanceSettings from './screens/settings/client/AppearanceSettings';
import ContactsSettings from './screens/settings/library/ContactsSettings';
import KeysSettings from './screens/settings/library/KeysSetting';
import LocationSettings from './screens/settings/library/LocationSettings';
import SecuritySettings from './screens/settings/library/SecuritySettings';
import SharingSettings from './screens/settings/library/SharingSettings';
import SyncSettings from './screens/settings/library/SyncSettings';
import TagsSettings from './screens/settings/library/TagsSettings';
import ExperimentalSettings from './screens/settings/node/ExperimentalSettings';
import GeneralSettings from './screens/settings/node/GeneralSettings';
import LibrarySettings from './screens/settings/node/LibrariesSettings';

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
					<Route path={'settings'} element={<SettingsScreen />}>
						<Route index element={<GeneralSettings />} />
						<Route path="general" element={<GeneralSettings />} />
						<Route path="appearance" element={<AppearanceSettings />} />
						<Route path="contacts" element={<ContactsSettings />} />
						<Route path="experimental" element={<ExperimentalSettings />} />
						<Route path="keys" element={<KeysSettings />} />
						<Route path="library" element={<LibrarySettings />} />
						<Route path="security" element={<SecuritySettings />} />
						<Route path="locations" element={<LocationSettings />} />
						<Route path="sharing" element={<SharingSettings />} />
						<Route path="sync" element={<SyncSettings />} />
						<Route path="tags" element={<TagsSettings />} />
					</Route>
					<Route path="explorer/:id" element={<ExplorerScreen />} />
					<Route path="tag/:id" element={<TagScreen />} />
					<Route path="*" element={<NotFound />} />
				</Route>
			</Routes>
		</>
	);
}
