import loadable from '@loadable/component';
import { useCurrentLibrary, useInvalidateQuery } from '@sd/client';
import { Suspense } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';

import { AppLayout } from './AppLayout';
import { useKeybindHandler } from './hooks/useKeyboardHandler';

// Using React.lazy breaks hot reload so we don't use it.
const DebugScreen = loadable(() => import('./screens/Debug'));
const SettingsScreen = loadable(() => import('./screens/settings/Settings'));
const TagExplorer = loadable(() => import('./screens/TagExplorer'));
const PhotosScreen = loadable(() => import('./screens/Photos'));
const OverviewScreen = loadable(() => import('./screens/Overview'));
const ContentScreen = loadable(() => import('./screens/Content'));
const LocationExplorer = loadable(() => import('./screens/LocationExplorer'));
const OnboardingScreen = loadable(() => import('./components/onboarding/Onboarding'));
const NotFound = loadable(() => import('./NotFound'));

const AppearanceSettings = loadable(() => import('./screens/settings/client/AppearanceSettings'));
const ExtensionSettings = loadable(() => import('./screens/settings/client/ExtensionsSettings'));
const GeneralSettings = loadable(() => import('./screens/settings/client/GeneralSettings'));
const KeybindingSettings = loadable(() => import('./screens/settings/client/KeybindingSettings'));
const PrivacySettings = loadable(() => import('./screens/settings/client/PrivacySettings'));
const AboutSpacedrive = loadable(() => import('./screens/settings/info/AboutSpacedrive'));
const Changelog = loadable(() => import('./screens/settings/info/Changelog'));
const Support = loadable(() => import('./screens/settings/info/Support'));
const ContactsSettings = loadable(() => import('./screens/settings/library/ContactsSettings'));
const KeysSettings = loadable(() => import('./screens/settings/library/KeysSetting'));
const LibraryGeneralSettings = loadable(
	() => import('./screens/settings/library/LibraryGeneralSettings')
);
const LocationSettings = loadable(() => import('./screens/settings/library/LocationSettings'));
const NodesSettings = loadable(() => import('./screens/settings/library/NodesSettings'));
const SecuritySettings = loadable(() => import('./screens/settings/library/SecuritySettings'));
const SharingSettings = loadable(() => import('./screens/settings/library/SharingSettings'));
const SyncSettings = loadable(() => import('./screens/settings/library/SyncSettings'));
const TagsSettings = loadable(() => import('./screens/settings/library/TagsSettings'));
const ExperimentalSettings = loadable(() => import('./screens/settings/node/ExperimentalSettings'));
const LibrarySettings = loadable(() => import('./screens/settings/node/LibrariesSettings'));
const P2PSettings = loadable(() => import('./screens/settings/node/P2PSettings'));

export function AppRouter() {
	const { library } = useCurrentLibrary();

	useKeybindHandler();
	useInvalidateQuery();

	return (
		<Suspense fallback={<p>Loading...</p>}>
			<Routes>
				<Route path="onboarding" element={<OnboardingScreen />} />
				<Route element={<AppLayout />}>
					{/* As we are caching the libraries in localStore so this *shouldn't* result is visual problems unless something else is wrong */}
					{library === undefined ? (
						<Route
							path="*"
							element={
								<h1 className="text-white p-4">
									Please select or create a library in the sidebar.
								</h1>
							}
						/>
					) : (
						<>
							<Route index element={<Navigate to="/overview" />} />
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
		</Suspense>
	);
}
