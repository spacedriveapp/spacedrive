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
  Eject,
  EjectSimple,
  Folder,
  HandGrabbing,
  HardDrive,
  HardDrives,
  MonitorPlay,
  Package,
  Planet
} from 'phosphor-react';
import React, { useEffect } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useLocations } from '../../store/locations';
import { Dropdown } from '../primative/Dropdown';
import { DefaultProps } from '../primative/types';

// const tabs: Record<string, { name: string; icon: any; uri: string }[]> = {
//   '': [
//     { name: 'Spaces', icon: CirclesFour, uri: '/spaces' },
//     { name: 'Explorer', icon: Folder, uri: '/explorer' },
//     { name: 'Media', icon: MonitorPlay, uri: '/settings' }
//   ],
// };

interface SidebarProps extends DefaultProps {}

const SidebarLink = (props: NavLinkProps) => (
  <NavLink
    className="max-w mb-[2px] text-gray-550 dark:text-gray-150 rounded-md px-2 py-1 flex flex-row items-center hover:bg-gray-100 dark:hover:bg-gray-600 text-sm"
    activeClassName="!bg-primary !text-white hover:bg-primary dark:hover:bg-primary"
    {...props}
  >
    {props.children}
  </NavLink>
);

const Icon = ({ component: Icon }: any) => <Icon weight="bold" className="w-4 h-4 mr-2 " />;

const Heading: React.FC<{}> = ({ children }) => (
  <div className="text-xs font-semibold text-gray-300 ml-1 mb-1 mt-5">{children}</div>
);

export const Sidebar: React.FC<SidebarProps> = (props) => {
  const locations = useLocations();

  console.log({ locations });

  return (
    <div className="w-46 flex flex-col flex-wrap flex-shrink-0 min-h-full bg-gray-50 dark:bg-gray-650  border-gray-100 border-r dark:border-gray-600 px-3  space-y-0.5">
      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className:
            'mb-1 !bg-gray-50 border-gray-150 hover:!bg-gray-100 flex-shrink-0 w-[175px] dark:!bg-gray-600 dark:hover:!bg-gray-550 dark:!border-gray-550 dark:hover:!border-gray-500',
          variant: 'gray'
        }}
        // buttonIcon={<Book weight="bold" className="w-4 h-4 mt-0.5 mr-1" />}
        buttonText="Jamie's Library"
        items={[[{ name: `Jamie's Library` }, { name: 'Subto' }], [{ name: 'Add Library' }]]}
      />

      <div>
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
            <SidebarLink key={index} to={`/explorer/${location.name}`}>
              <Icon component={ServerIcon} />
              {location.name}
              <div className="flex-grow" />
              {location.is_removable && <Icon component={EjectSimple} />}
            </SidebarLink>
          );
        })}
      </div>
    </div>
  );
};
