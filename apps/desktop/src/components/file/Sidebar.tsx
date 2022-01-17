import {
  BookOpenIcon,
  CogIcon,
  CollectionIcon,
  CubeTransparentIcon,
  DatabaseIcon,
  FolderIcon,
  LibraryIcon,
  PhotographIcon,
  PlusIcon,
  ServerIcon
} from '@heroicons/react/solid';
import { appWindow } from '@tauri-apps/api/window';
import clsx from 'clsx';
import {
  Book,
  Camera,
  Circle,
  CirclesFour,
  Eject,
  EjectSimple,
  Folder,
  HandGrabbing,
  HardDrive,
  HardDrives,
  MonitorPlay,
  Package,
  Planet,
  Plus
} from 'phosphor-react';
import React, { useEffect } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useLocations } from '../../store/locations';
import { TrafficLights } from '../os/TrafficLights';
import { Button } from '../primitive';
import { Dropdown } from '../primitive/Dropdown';
import { DefaultProps } from '../primitive/types';

interface SidebarProps extends DefaultProps {}

const SidebarLink = (props: NavLinkProps) => (
  <NavLink
    {...props}
    className={clsx(
      'max-w mb-[2px] text-gray-550 dark:text-gray-150 rounded px-2 py-1 flex flex-row flex-grow items-center hover:bg-gray-100 dark:hover:bg-gray-600 text-sm',
      props.className
    )}
    activeClassName="!bg-primary !text-white hover:bg-primary dark:hover:bg-primary"
  >
    {props.children}
  </NavLink>
);

const Icon = ({ component: Icon, ...props }: any) => (
  <Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{}> = ({ children }) => (
  <div className="mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300">{children}</div>
);

export const Sidebar: React.FC<SidebarProps> = (props) => {
  const locations = useLocations();
  return (
    <div className="flex flex-col flex-wrap flex-shrink-0 min-h-full px-3 py-1 border-r border-gray-100 w-46 bg-gray-50 dark:bg-gray-850 dark:border-gray-600">
      <div data-tauri-drag-region className="mt-2 mb-1 -ml-1 ">
        <TrafficLights
          onClose={appWindow.close}
          onFullscreen={appWindow.maximize}
          onMinimize={appWindow.minimize}
          className="p-1.5"
        />
      </div>
      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className: `mb-1 shadow-xs rounded flex-shrink-0 w-[175px] 
            !bg-gray-50 
            border-gray-150 
            hover:!bg-gray-1000 
            
            dark:!bg-gray-550 
            dark:hover:!bg-gray-550

            dark:!border-gray-550 
            dark:hover:!border-gray-500`,
          variant: 'gray'
        }}
        // buttonIcon={<Book weight="bold" className="w-4 h-4 mt-0.5 mr-1" />}
        buttonText="Jamie's Library"
        items={[
          [{ name: `Jamie's Library`, selected: true }, { name: 'Subto' }],
          [
            { name: 'Library Settings', icon: CogIcon },
            { name: 'Add Library', icon: PlusIcon }
          ]
        ]}
      />

      <div className="pt-1">
        <SidebarLink to="/overview">
          <Icon component={Planet} />
          Overview
        </SidebarLink>
        <SidebarLink to="/spaces">
          <Icon component={CirclesFour} />
          Spaces
        </SidebarLink>
        <SidebarLink to="/explorer">
          <Icon component={Folder} />
          Explorer
        </SidebarLink>
        <SidebarLink to="/settings">
          <Icon component={MonitorPlay} />
          Media
        </SidebarLink>
      </div>
      <div>
        <Heading>Locations</Heading>
        {locations.map((location, index) => {
          return (
            <div className="flex flex-row items-center">
              <SidebarLink className="relative group" key={index} to={`/explorer/${location.name}`}>
                <Icon component={ServerIcon} />
                {location.name}
                <div className="flex-grow" />
                {location.is_removable && (
                  <Button
                    noBorder
                    size="sm"
                    className="w-7 h-7 top-0 right-0 absolute !bg-transparent group-hover:bg-gray-600 dark:hover:!bg-gray-550 !transition-none items-center !rounded-l-none"
                  >
                    <Icon className="w-3 h-3 mr-0 " component={EjectSimple} />
                  </Button>
                )}
              </SidebarLink>
            </div>
          );
        })}
      </div>
    </div>
  );
};
