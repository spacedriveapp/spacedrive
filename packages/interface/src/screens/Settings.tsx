// import { dummyIFile, FileList } from '../components/file/FileList';
import {
	CloudIcon,
	CogIcon,
	KeyIcon,
	LockClosedIcon,
	PhotographIcon,
	TagIcon,
	TerminalIcon,
	UsersIcon
} from '@heroicons/react/outline';
import clsx from 'clsx';
import { Book, Database, HardDrive, PaintBrush } from 'phosphor-react';
import React from 'react';
import { Outlet, Route, Routes } from 'react-router-dom';

import { SidebarLink } from '../components/file/Sidebar';
import { Modal } from '../components/layout/Modal';
import SlideUp from '../components/transitions/SlideUp';
import AppearanceSettings from './settings/AppearanceSettings';
import ContactsSettings from './settings/ContactsSettings';
import ExperimentalSettings from './settings/ExperimentalSettings';
import GeneralSettings from './settings/GeneralSettings';
import KeysSettings from './settings/KeysSetting';
import LibrarySettings from './settings/LibrarySettings';
import LocationSettings from './settings/LocationSettings';
import SecuritySettings from './settings/SecuritySettings';
import SharingSettings from './settings/SharingSettings';
import SyncSettings from './settings/SyncSettings';
import TagsSettings from './settings/TagsSettings';

//@ts-ignore
// import { Spline } from 'react-spline';
// import WINDOWS_SCENE from '../assets/spline/scene.json';

const Icon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{ className?: string; children: string }> = ({ children, className }) => (
	<div className={clsx('mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300', className)}>
		{children}
	</div>
);

export function SettingsRoutes({ modal = false }) {
	return (
		<SlideUp>
			<Routes>
				<Route
					path={modal ? '/settings' : '/'}
					element={modal ? <Modal children={<SettingsScreen />} /> : <SettingsScreen />}
				>
					<Route index element={<GeneralSettings />} />

					<Route path="appearance" element={<AppearanceSettings />} />
					<Route path="contacts" element={<ContactsSettings />} />
					<Route path="experimental" element={<ExperimentalSettings />} />
					<Route path="general" element={<GeneralSettings />} />
					<Route path="keys" element={<KeysSettings />} />
					<Route path="library" element={<LibrarySettings />} />
					<Route path="security" element={<SecuritySettings />} />
					<Route path="locations" element={<LocationSettings />} />
					<Route path="sharing" element={<SharingSettings />} />
					<Route path="sync" element={<SyncSettings />} />
					<Route path="tags" element={<TagsSettings />} />
				</Route>
			</Routes>
		</SlideUp>
	);
}

export const SettingsScreen: React.FC<{}> = () => {
	return (
		<div className="flex flex-row w-full">
			<div className="h-full border-r max-w-[200px] flex-shrink-0 border-gray-100 w-60 dark:border-gray-550">
				<div data-tauri-drag-region className="w-full h-7" />
				<div className="p-5 pt-0">
					<Heading className="mt-0">Client</Heading>
					<SidebarLink to="/settings/general">
						<Icon component={CogIcon} />
						General
					</SidebarLink>
					<SidebarLink to="/settings/security">
						<Icon component={LockClosedIcon} />
						Security
					</SidebarLink>
					<SidebarLink to="/settings/appearance">
						<Icon component={PaintBrush} />
						Appearance
					</SidebarLink>
					<SidebarLink to="/settings/experimental">
						<Icon component={TerminalIcon} />
						Experimental
					</SidebarLink>

					<Heading>Library</Heading>
					<SidebarLink to="/settings/library">
						<Icon component={Database} />
						Database
					</SidebarLink>
					<SidebarLink to="/settings/locations">
						<Icon component={HardDrive} />
						Locations
					</SidebarLink>

					<SidebarLink to="/settings/keys">
						<Icon component={KeyIcon} />
						Keys
					</SidebarLink>
					<SidebarLink to="/settings/tags">
						<Icon component={TagIcon} />
						Tags
					</SidebarLink>

					<Heading>Cloud</Heading>
					<SidebarLink to="/settings/sync">
						<Icon component={CloudIcon} />
						Sync
					</SidebarLink>
					<SidebarLink to="/settings/contacts">
						<Icon component={UsersIcon} />
						Contacts
					</SidebarLink>
				</div>
			</div>
			<div className="w-full">
				<div data-tauri-drag-region className="w-full h-7" />
				<div className="flex flex-grow-0 w-full h-full max-h-screen custom-scroll page-scroll">
					<div className="flex flex-grow px-12 pb-5">
						<Outlet />
						<div className="block h-20" />
					</div>
				</div>
			</div>
		</div>
	);
};
