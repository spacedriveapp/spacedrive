import { Navigate, Route, RouteProps } from 'react-router-dom';
import { lazyEl } from '~/util';
import SettingsSubPageScreen from './SettingsSubPage';
import LocationsSettings from './library/LocationsSettings';
import EditLocation from './library/location/EditLocation';

const routes: RouteProps[] = [
	{ index: true, element: <Navigate to="general" relative="route" /> },
	{ path: 'general', element: lazyEl(() => import('./client/GeneralSettings')) },
	{ path: 'appearance', element: lazyEl(() => import('./client/AppearanceSettings')) },
	{ path: 'keybindings', element: lazyEl(() => import('./client/KeybindingSettings')) },
	{ path: 'extensions', element: lazyEl(() => import('./client/ExtensionsSettings')) },
	{ path: 'p2p', element: lazyEl(() => import('./node/P2PSettings')) },
	{ path: 'contacts', element: lazyEl(() => import('./library/ContactsSettings')) },
	{ path: 'experimental', element: lazyEl(() => import('./node/ExperimentalSettings')) },
	{ path: 'keys', element: lazyEl(() => import('./library/KeysSetting')) },
	{ path: 'libraries', element: lazyEl(() => import('./node/LibrariesSettings')) },
	{ path: 'security', element: lazyEl(() => import('./library/SecuritySettings')) },
	{ path: 'locations', element: lazyEl(() => import('./library/LocationsSettings')) },
	{ path: 'sharing', element: lazyEl(() => import('./library/SharingSettings')) },
	{ path: 'sync', element: lazyEl(() => import('./library/SyncSettings')) },
	{ path: 'tags', element: lazyEl(() => import('./library/TagsSettings')) },
	{ path: 'library', element: lazyEl(() => import('./library/LibraryGeneralSettings')) },
	{ path: 'tags', element: lazyEl(() => import('./library/TagsSettings')) },
	{ path: 'nodes', element: lazyEl(() => import('./library/NodesSettings')) },
	{ path: 'privacy', element: lazyEl(() => import('./client/PrivacySettings')) },
	{ path: 'about', element: lazyEl(() => import('./info/AboutSpacedrive')) },
	{ path: 'changelog', element: lazyEl(() => import('./info/Changelog')) },
	{ path: 'support', element: lazyEl(() => import('./info/Support')) }
];

export default (
	<>
		{routes.map((route) => (
			<Route key={route.path} {...route} />
		))}
		{/* Skipping implementing via routes object due to a lack of understanding on how to accomplish the below route setup with this new approach, feel free to fix Brendan */}
		<Route path="locations" element={<SettingsSubPageScreen />}>
			<Route index element={<LocationsSettings />} />
			<Route path="location/:id" element={<EditLocation />} />
		</Route>
	</>
);
