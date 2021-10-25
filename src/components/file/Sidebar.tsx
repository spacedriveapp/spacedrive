import {
  CogIcon,
  CollectionIcon,
  CubeTransparentIcon,
  DatabaseIcon,
  PhotographIcon,
  ServerIcon
} from '@heroicons/react/solid';
import {
  Book,
  Camera,
  Circle,
  CirclesFour,
  Folder,
  HandGrabbing,
  HardDrive,
  HardDrives,
  MonitorPlay,
  Package,
  Planet
} from 'phosphor-react';
import React from 'react';
import { NavLink } from 'react-router-dom';
import { Dropdown } from '../primative/Dropdown';
import { DefaultProps } from '../primative/types';

const tabs = {
  '': [
    { name: 'Spaces', icon: CirclesFour, uri: '/spaces' },
    { name: 'Explorer', icon: Folder, uri: '/explorer' },
    { name: 'Media', icon: MonitorPlay, uri: '/photos' }
  ],
  'Locations': [
    { name: 'Macintosh HD', icon: HardDrive, uri: '/x' },
    { name: 'LaCie 2TB', icon: HardDrive, uri: '/xs' },
    { name: 'Seagate 16TB', icon: HardDrive, uri: '/xss' }
  ]
};

interface SidebarProps extends DefaultProps {}

export const Sidebar: React.FC<SidebarProps> = (props) => {
  return (
    <div className="w-46 flex flex-col flex-wrap flex-shrink-0 min-h-full bg-gray-50 dark:bg-gray-650  border-gray-100 border-r dark:border-gray-700 px-3  space-y-0.5">
      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className:
            'mb-1 !bg-gray-50 border-gray-150 hover:!bg-gray-100 flex-shrink-0 w-[175px] dark:bg-gray-550 dark:hover:!bg-gray-500 dark:hover:!border-gray-450',
          variant: 'gray'
        }}
        // buttonIcon={<Book weight="bold" className="w-4 h-4 mt-0.5 mr-1" />}
        buttonText="Jamie's Library"
        items={[[{ name: `Jamie's Library` }, { name: 'Subto' }], [{ name: 'Add Library' }]]}
      />

      {Object.keys(tabs).map((name) => {
        return (
          <div className="">
            {name && (
              <div className="text-xs font-semibold text-gray-300 ml-1 mb-1 mt-5">{name}</div>
            )}
            {tabs[name as keyof typeof tabs].map((button, index) => (
              <NavLink
                key={index}
                className="max-w text-gray-550 dark:text-gray-150 rounded-md px-2 py-1 flex flex-row items-center hover:bg-gray-100 dark:hover:bg-gray-600 text-sm"
                activeClassName="!bg-primary !text-white hover:bg-primary dark:hover:bg-primary"
                to={button.uri}
              >
                {button.icon && <button.icon weight="bold" className="w-4 h-4 mr-2 " />}
                {button.name}
              </NavLink>
            ))}
          </div>
        );
      })}
    </div>
  );
};
