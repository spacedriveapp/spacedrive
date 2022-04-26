import { LockClosedIcon } from '@heroicons/react/outline';
import { CogIcon, EyeOffIcon, PlusIcon, ServerIcon } from '@heroicons/react/solid';
import clsx from 'clsx';
import { CirclesFour, Code, EjectSimple, MonitorPlay, Planet } from 'phosphor-react';
import React, { useContext, useEffect, useState } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { TrafficLights } from '../os/TrafficLights';
import { Button, Dropdown } from '@sd/ui';
import { DefaultProps } from '../primitive/types';
import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import RunningJobsWidget from '../jobs/RunningJobsWidget';
import { AppPropsContext } from '../../App';

import { ReactComponent as Folder } from '../../assets/svg/folder.svg';
import { ReactComponent as FolderWhite } from '../../assets/svg/folder-white.svg';

interface SidebarProps extends DefaultProps {}

export const SidebarLink = (props: NavLinkProps & { children: React.ReactNode }) => (
  <NavLink {...props}>
    {({ isActive }) => (
      <span
        className={clsx(
          'max-w mb-[2px] text-gray-550 dark:text-gray-150 rounded px-2 py-1 flex flex-row flex-grow items-center hover:bg-gray-100 dark:hover:bg-gray-600 text-sm',
          { '!bg-primary !text-white hover:bg-primary dark:hover:bg-primary': isActive },
          props.className
        )}
      >
        {props.children}
      </span>
    )}
  </NavLink>
);

const Icon = ({ component: Icon, ...props }: any) => (
  <Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className="mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300">{children}</div>
);

export function MacOSTrafficLights() {
  const appPropsContext = useContext(AppPropsContext);

  return (
    <div data-tauri-drag-region className="mt-2 mb-1 -ml-1 ">
      <TrafficLights
        onClose={appPropsContext?.onClose}
        onFullscreen={appPropsContext?.onFullscreen}
        onMinimize={appPropsContext?.onMinimize}
        className="p-1.5 z-50 absolute"
      />
    </div>
  );
}

export const Sidebar: React.FC<SidebarProps> = (props) => {
  const appPropsContext = useContext(AppPropsContext);
  const { data: locations } = useBridgeQuery('SysGetLocations');
  const { data: clientState } = useBridgeQuery('ClientGetState');

  const { mutate: createLocation } = useBridgeCommand('LocCreate');

  const tags = [
    { id: 1, name: 'Keepsafe', color: '#FF6788' },
    { id: 2, name: 'OBS', color: '#BF88FF' },
    { id: 2, name: 'BlackMagic', color: '#F0C94A' },
    { id: 2, name: 'Camera Roll', color: '#00F0DB' },
    { id: 2, name: 'Spacedrive', color: '#00F079' }
  ];

  return (
    <div className="flex flex-col flex-grow-0 flex-shrink-0 w-48 min-h-full px-3 pb-1 overflow-x-hidden overflow-y-scroll border-r border-gray-100 bg-gray-50 dark:bg-gray-850 dark:border-gray-600">
      {appPropsContext?.platform === 'macOS' ? (
        <>
          <MacOSTrafficLights /> <div className="mt-6" />
        </>
      ) : null}

      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className: `flex w-full text-left max-w-full mb-1 mt-1 shadow-xs rounded 
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
        buttonText={clientState?.client_name || 'Loading...'}
        items={[
          [{ name: clientState?.client_name || '', selected: true }, { name: 'Private Library' }],
          [
            { name: 'Library Settings', icon: CogIcon },
            { name: 'Add Library', icon: PlusIcon },
            { name: 'Lock', icon: LockClosedIcon },
            { name: 'Hide', icon: EyeOffIcon }
          ]
        ]}
      />

      <div className="pt-1">
        <SidebarLink to="/overview">
          <Icon component={Planet} />
          Overview
        </SidebarLink>
        <SidebarLink to="content">
          <Icon component={CirclesFour} />
          Content
        </SidebarLink>
        <SidebarLink to="debug">
          <Icon component={Code} />
          Debug
        </SidebarLink>
        {/* <SidebarLink to="explorer">
          <Icon component={MonitorPlay} />
          Explorer
        </SidebarLink> */}
      </div>
      <div>
        <Heading>Locations</Heading>
        {locations?.map((location, index) => {
          return (
            <div key={index} className="flex flex-row items-center">
              <NavLink
                className="'relative w-full group'"
                to={{
                  pathname: `explorer/${location.id}`
                }}
              >
                {({ isActive }) => (
                  <span
                    className={clsx(
                      'max-w mb-[2px] text-gray-550 dark:text-gray-150 rounded px-2 py-1 flex flex-row flex-grow items-center hover:bg-gray-100 dark:hover:bg-gray-600 text-sm',
                      {
                        '!bg-primary !text-white hover:bg-primary dark:hover:bg-primary': isActive
                      }
                    )}
                  >
                    <div className="w-[18px] mr-2 -mt-0.5">
                      <FolderWhite className={clsx(!isActive && 'hidden')} />
                      <Folder className={clsx(isActive && 'hidden')} />
                    </div>
                    {location.name}
                    <div className="flex-grow" />
                  </span>
                )}
              </NavLink>
            </div>
          );
        })}

        <button
          onClick={() => {
            appPropsContext?.openDialog({ directory: true }).then((result) => {
              createLocation({ path: result });
            });
          }}
          className="w-full px-2 py-1.5 mt-1 text-xs font-bold text-center text-gray-400 dark:text-gray-500 border border-dashed rounded border-transparent cursor-normal border-gray-350 dark:border-gray-550 hover:dark:border-gray-500 transition"
        >
          Add Location
        </button>
      </div>
      <div>
        <Heading>Tags</Heading>
        <div className="mb-2">
          {tags.map((tag, index) => (
            <SidebarLink key={index} to="/" className="">
              <div
                className="w-[12px] h-[12px] rounded-full"
                style={{ backgroundColor: tag.color }}
              />
              <span className="ml-2 text-sm">{tag.name}</span>
            </SidebarLink>
          ))}
        </div>
      </div>
      <div className="flex-grow" />
      <RunningJobsWidget />
      {/* <div className="flex w-full">
      </div> */}
      <div className="mb-2">
        <NavLink to="/settings/general">
          {({ isActive }) => (
            <Button
              variant={isActive ? 'default' : 'default'}
              className={clsx(
                'px-[4px]'
                // isActive && '!bg-gray-550'
              )}
            >
              <CogIcon className="w-5 h-5" />
            </Button>
          )}
        </NavLink>
      </div>
    </div>
  );
};
