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
import React from 'react';
// import { dummyIFile, FileList } from '../components/file/FileList';
import { SidebarLink } from '../components/file/Sidebar';
import { Book, Database, HardDrive, PaintBrush } from 'phosphor-react';
import clsx from 'clsx';
import { Outlet } from 'react-router-dom';

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

export const SettingsScreen: React.FC<{}> = () => {
  return (
    <div className="flex flex-row w-full">
      <div className="h-full border-r border-gray-100 w-60 dark:border-gray-550">
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
      <div className="">
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
