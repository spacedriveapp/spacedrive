import {
  CloudIcon,
  CogIcon,
  KeyIcon,
  LockClosedIcon,
  PhotographIcon,
  TagIcon,
  UsersIcon
} from '@heroicons/react/solid';
import React from 'react';
// import { dummyIFile, FileList } from '../components/file/FileList';
import { SidebarLink } from '../components/file/Sidebar';
import { HardDrive, PaintBrush } from 'phosphor-react';
import clsx from 'clsx';
import { Outlet } from 'react-router-dom';

//@ts-ignore
// import { Spline } from 'react-spline';
// import WINDOWS_SCENE from '../assets/spline/scene.json';

const Icon = ({ component: Icon, ...props }: any) => (
  <Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{ className?: string }> = ({ children, className }) => (
  <div className={clsx('mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300', className)}>
    {children}
  </div>
);

export const SettingsScreen: React.FC<{}> = () => {
  return (
    <div className="flex flex-row w-full">
      <div className="h-full p-5 border-r border-gray-100 w-60 dark:border-gray-550">
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

        <Heading>Library</Heading>
        <SidebarLink to="/settings/locations">
          <Icon component={HardDrive} />
          Locations
        </SidebarLink>
        <SidebarLink to="/settings/media">
          <Icon component={PhotographIcon} />
          Media
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
      <div className="flex flex-grow-0 w-full h-full max-h-screen overflow-y-scroll">
        <div className="flex flex-grow px-12 py-5">
          <Outlet />
          <div className="block h-20" />
        </div>
      </div>
    </div>
  );
};
