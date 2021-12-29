import { ChevronLeftIcon, ChevronRightIcon, CogIcon } from '@heroicons/react/outline';
import clsx from 'clsx';
import {
  ArrowsLeftRight,
  Cloud,
  Columns,
  FolderPlus,
  HouseSimple,
  Key,
  SquaresFour,
  Tag,
  TerminalWindow
} from 'phosphor-react';
import React from 'react';
import { useExplorerStore } from '../../store/explorer';
import { TrafficLights } from '../os/TrafficLights';
import { Button, ButtonProps, Input } from '../primitive';
import { Shortcut } from '../primitive/Shortcut';
import { DefaultProps } from '../primitive/types';
import { appWindow } from '@tauri-apps/api/window';
import { HeartIcon } from '@heroicons/react/solid';
import { invoke } from '@tauri-apps/api';

export interface TopBarProps extends DefaultProps {}
export interface TopBarButtonProps extends ButtonProps {
  icon: any;
  group?: boolean;
  active?: boolean;
  left?: boolean;
  right?: boolean;
}

const TopBarButton: React.FC<TopBarButtonProps> = ({ icon: Icon, ...props }) => {
  return (
    <button
      {...props}
      className={clsx(
        'mr-[1px] py-0.5 px-0.5 text-md font-medium hover:bg-gray-150 dark:transparent dark:hover:bg-gray-600 dark:active:bg-gray-500 rounded-md transition-colors duration-100',
        {
          'rounded-r-none rounded-l-none': props.group && !props.left && !props.right,
          'rounded-r-none': props.group && props.left,
          'rounded-l-none': props.group && props.right,
          'dark:bg-gray-450 dark:hover:bg-gray-450 dark:active:bg-gray-450': props.active
        },
        props.className
      )}
    >
      <Icon weight={'regular'} className="m-0.5 w-5 h-5 text-gray-450 dark:text-gray-150" />
    </button>
  );
};

export const TopBar: React.FC<TopBarProps> = (props) => {
  const [goBack] = useExplorerStore((state) => [state.goBack]);
  return (
    <>
      <div
        data-tauri-drag-region
        className="flex h-[2.95rem] -mt-0.5 max-w z-50 rounded-t-2xl flex-shrink-0 items-center border-b bg-gray-50 dark:bg-gray-650 border-gray-100 dark:border-gray-600 !bg-opacity-60 backdrop-blur "
      >
        <div className="ml-1 mr-32">
          <TrafficLights
            onClose={appWindow.close}
            onFullscreen={appWindow.maximize}
            onMinimize={appWindow.minimize}
            className="p-1.5"
          />
        </div>
        <TopBarButton icon={ChevronLeftIcon} onClick={goBack} />
        <TopBarButton icon={ChevronRightIcon} />
        {/* <div className="flex mx-8 space-x-[1px]">
          <TopBarButton active group left icon={List} />
          <TopBarButton group icon={Columns} />
          <TopBarButton group right icon={SquaresFour} />
        </div> */}
        <div className="flex-grow"></div>
        <div className="flex mx-8 space-x-2">
          <TopBarButton icon={Tag} />
          <TopBarButton icon={FolderPlus} />
          <TopBarButton icon={TerminalWindow} />
        </div>
        <div className="relative flex h-7">
          <input
            placeholder="Search"
            className="w-32 h-[30px] focus:w-52 text-sm p-3 rounded-lg outline-none focus:ring-2  placeholder-gray-400 dark:placeholder-gray-500 bg-gray-50 border border-gray-250 dark:bg-gray-700 dark:border-gray-600 focus:ring-gray-100 dark:focus:ring-gray-600 transition-all"
          />
          <div className="space-x-1 absolute top-[2px] right-1">
            <Shortcut chars="⌘S" />
            {/* <Shortcut chars="S" /> */}
          </div>
        </div>
        <div className="flex mx-8 space-x-2">
          <TopBarButton icon={Key} />
          <TopBarButton icon={Cloud} />
          <TopBarButton icon={ArrowsLeftRight} onClick={() => invoke("start_watcher", { path: "/Users/jamie/Downloads"})} />
        </div>
        <div className="flex-grow"></div>
        <div className="flex-grow"></div>
        <TopBarButton className="mr-[8px]" icon={CogIcon} />
      </div>
      {/* <div className="h-[1px] flex-shrink-0 max-w bg-gray-200 dark:bg-gray-700" /> */}
    </>
  );
};
