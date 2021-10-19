import {
  CogIcon,
  CollectionIcon,
  CubeTransparentIcon,
  DatabaseIcon,
  PhotographIcon,
  ServerIcon
} from '@heroicons/react/solid';
import { Planet } from 'phosphor-react';
import React from 'react';
import { NavLink } from 'react-router-dom';
import { Dropdown } from '../primative/Dropdown';
import { DefaultProps } from '../primative/types';

const tabs = [
  // { name: 'Spaces', icon: Planet, uri: '/spaces' },
  { name: 'Explorer', icon: CubeTransparentIcon, uri: '/explorer' },
  { name: 'Photos', icon: PhotographIcon, uri: '/photos' },
  { name: 'Storage', icon: ServerIcon, uri: '/settings' }
  // { name: 'Settings', icon: CogIcon, uri: '/settings' }
];

interface SidebarProps extends DefaultProps {}

export const Sidebar: React.FC<SidebarProps> = (props) => {
  return (
    <div className="w-46 flex flex-col flex-wrap flex-shrink-0 min-h-full bg-gray-50 dark:bg-gray-850 border-gray-100 border-r dark:border-gray-700 px-3  space-y-0.5">
      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className: 'mb-1 shadow flex-shrink-0 w-[175px] dark:bg-gray-650',
          variant: 'gray'
        }}
        buttonText="Jamie's Library"
        items={[[{ name: `Jamie's Library` }, { name: 'Subto' }], [{ name: 'Add Library' }]]}
      />

      {tabs.map((button, index) => (
        <NavLink
          key={index}
          className="max-w rounded px-2 py-1 flex flex-row items-center hover:bg-gray-200 dark:hover:bg-gray-600 text-sm"
          activeClassName="bg-gray-200 hover:bg-gray-200 dark:bg-gray-500 dark:hover:bg-gray-500"
          to={button.uri}
        >
          {button.icon && <button.icon className="w-4 h-4 mr-2" />}
          {button.name}
        </NavLink>
      ))}
    </div>
  );
};
