import { useBridgeQuery } from '@sd/client';
import { useLibraryStore } from '@sd/client';
import React, { useEffect } from 'react';
import { Route, Routes, useLocation } from 'react-router-dom';

import { AppLayout } from './AppLayout';
import { NotFound } from './NotFound';
import { useLibraryState } from './hooks/useLibraryState';
import { ContentScreen } from './screens/Content';
import { DebugScreen } from './screens/Debug';
import { ExplorerScreen } from './screens/Explorer';
import { OverviewScreen } from './screens/Overview';
import { PhotosScreen } from './screens/Photos';
import { RedirectPage } from './screens/Redirect';
import { TagScreen } from './screens/Tag';
import { CurrentLibrarySettings } from './screens/settings/CurrentLibrarySettings';
import { SettingsScreen } from './screens/settings/Settings';
import AppearanceSettings from './screens/settings/client/AppearanceSettings';
import GeneralSettings from './screens/settings/client/GeneralSettings';
import ContactsSettings from './screens/settings/library/ContactsSettings';
import KeysSettings from './screens/settings/library/KeysSetting';
import LibraryGeneralSettings from './screens/settings/library/LibraryGeneralSettings';
import LocationSettings from './screens/settings/library/LocationSettings';
import SecuritySettings from './screens/settings/library/SecuritySettings';
import SharingSettings from './screens/settings/library/SharingSettings';
import SyncSettings from './screens/settings/library/SyncSettings';
import TagsSettings from './screens/settings/library/TagsSettings';
import ExperimentalSettings from './screens/settings/node/ExperimentalSettings';
import LibrarySettings from './screens/settings/node/LibrariesSettings';
import NodesSettings from './screens/settings/node/NodesSettings';
import P2PSettings from './screens/settings/node/P2PSettings';

export function AppRouter() {
	let location = useLocation();
	let state = location.state as { backgroundLocation?: Location };

	const libraryState = useLibraryStore();
	const { data: libraries } = useBridgeQuery('NodeGetLibraries');

	// TODO: This can be removed once we add a setup flow to the app
	useEffect(() => {
		if (libraryState.currentLibraryUuid === null && libraries && libraries.length > 0) {
			libraryState.switchLibrary(libraries[0].uuid);
		}
	}, [libraryState.currentLibraryUuid, libraries]);

	return (
		<>
			{libraryState.currentLibraryUuid === null ? (
				<>
					{/* TODO: Remove this when adding app setup flow */}
					<h1>No Library Loaded...</h1>
				</>
			) : (
				<Routes location={state?.backgroundLocation || location}>
					<Route path="/" element={<AppLayout />}>
						<Route index element={<RedirectPage to="/overview" />} />
						<Route path="overview" element={<OverviewScreen />} />
						<Route path="content" element={<ContentScreen />} />
						<Route path="photos" element={<PhotosScreen />} />
						<Route path="debug" element={<DebugScreen />} />
						<Route path={'library-settings'} element={<CurrentLibrarySettings />}>
							<Route index element={<LocationSettings />} />
							<Route path="general" element={<LibraryGeneralSettings />} />
							<Route path="locations" element={<LocationSettings />} />
							<Route path="tags" element={<TagsSettings />} />
							<Route path="keys" element={<KeysSettings />} />
						</Route>
						<Route path={'settings'} element={<SettingsScreen />}>
							<Route index element={<GeneralSettings />} />
							<Route path="general" element={<GeneralSettings />} />
							<Route path="appearance" element={<AppearanceSettings />} />
							<Route path="nodes" element={<NodesSettings />} />
							<Route path="p2p" element={<P2PSettings />} />
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
			)}
		</>
	);
}
