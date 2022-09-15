import { useCurrentLibrary, useInvalidateQuery } from '@sd/client';
import { Route, Routes } from 'react-router-dom';

import { AppLayout } from './AppLayout';
import { NotFound } from './NotFound';
import OnboardingScreen from './components/onboarding/Onboarding';
import { useKeybindHandler } from './hooks/useKeyboardHandler';
import { ContentScreen } from './screens/Content';
import { DebugScreen } from './screens/Debug';
import { LocationExplorer } from './screens/LocationExplorer';
import { OverviewScreen } from './screens/Overview';
import { PhotosScreen } from './screens/Photos';
import { RedirectPage } from './screens/Redirect';
import { TagExplorer } from './screens/TagExplorer';
import { SettingsScreen } from './screens/settings/Settings';
import AppearanceSettings from './screens/settings/client/AppearanceSettings';
import ExtensionSettings from './screens/settings/client/ExtensionsSettings';
import GeneralSettings from './screens/settings/client/GeneralSettings';
import KeybindingSettings from './screens/settings/client/KeybindingSettings';
import PrivacySettings from './screens/settings/client/PrivacySettings';
import AboutSpacedrive from './screens/settings/info/AboutSpacedrive';
import Changelog from './screens/settings/info/Changelog';
import Support from './screens/settings/info/Support';
import ContactsSettings from './screens/settings/library/ContactsSettings';
import KeysSettings from './screens/settings/library/KeysSetting';
import LibraryGeneralSettings from './screens/settings/library/LibraryGeneralSettings';
import LocationSettings from './screens/settings/library/LocationSettings';
import NodesSettings from './screens/settings/library/NodesSettings';
import SecuritySettings from './screens/settings/library/SecuritySettings';
import SharingSettings from './screens/settings/library/SharingSettings';
import SyncSettings from './screens/settings/library/SyncSettings';
import TagsSettings from './screens/settings/library/TagsSettings';
import ExperimentalSettings from './screens/settings/node/ExperimentalSettings';
import LibrarySettings from './screens/settings/node/LibrariesSettings';
import P2PSettings from './screens/settings/node/P2PSettings';

export function AppRouter() {
	const { library } = useCurrentLibrary();

	useKeybindHandler();
	useInvalidateQuery();

	return (
		<Routes>
			<Route path="onboarding" element={<OnboardingScreen />} />
			<Route element={<AppLayout />}>
				{/* As we are caching the libraries in localStore so this *shouldn't* result is visual problems unless something else is wrong */}
				{library === undefined ? (
					<Route
						path="*"
						element={
							<h1 className="text-white p-4">Please select or create a library in the sidebar.</h1>
						}
					/>
				) : (
					<>
						<Route index element={<RedirectPage to="/overview" />} />
						<Route path="overview" element={<OverviewScreen />} />
						<Route path="content" element={<ContentScreen />} />
						<Route path="photos" element={<PhotosScreen />} />
						<Route path="debug" element={<DebugScreen />} />
						<Route path={'settings'} element={<SettingsScreen />}>
							<Route index element={<GeneralSettings />} />
							<Route path="general" element={<GeneralSettings />} />
							<Route path="appearance" element={<AppearanceSettings />} />
							<Route path="keybindings" element={<KeybindingSettings />} />
							<Route path="extensions" element={<ExtensionSettings />} />
							<Route path="p2p" element={<P2PSettings />} />
							<Route path="contacts" element={<ContactsSettings />} />
							<Route path="experimental" element={<ExperimentalSettings />} />
							<Route path="keys" element={<KeysSettings />} />
							<Route path="libraries" element={<LibrarySettings />} />
							<Route path="security" element={<SecuritySettings />} />
							<Route path="locations" element={<LocationSettings />} />
							<Route path="sharing" element={<SharingSettings />} />
							<Route path="sync" element={<SyncSettings />} />
							<Route path="tags" element={<TagsSettings />} />
							<Route path="library" element={<LibraryGeneralSettings />} />
							<Route path="locations" element={<LocationSettings />} />
							<Route path="tags" element={<TagsSettings />} />
							<Route path="nodes" element={<NodesSettings />} />
							<Route path="keys" element={<KeysSettings />} />
							<Route path="privacy" element={<PrivacySettings />} />
							<Route path="about" element={<AboutSpacedrive />} />
							<Route path="changelog" element={<Changelog />} />
							<Route path="support" element={<Support />} />
						</Route>
						<Route path="location/:id" element={<LocationExplorer />} />
						<Route path="tag/:id" element={<TagExplorer />} />
						<Route path="*" element={<NotFound />} />
					</>
				)}
			</Route>
		</Routes>
	);
}
