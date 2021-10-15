import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CogIcon,
  HomeIcon,
  ViewBoardsIcon,
  ViewGridIcon,
  ViewListIcon
} from '@heroicons/react/outline';
import { HouseSimple } from 'phosphor-react';
import React from 'react';
import { TrafficLights } from '../os/TrafficLights';
import { Button, Input } from '../primative';
import { Shortcut } from '../primative/Shortcut';
import { DefaultProps } from '../primative/types';

export interface TopBarProps extends DefaultProps {}

export const TopBar: React.FC<TopBarProps> = (props) => {
  return (
    <>
      <div
        data-tauri-drag-region
        className="flex flex-shrink-0 h-10 max-w items-center bg-gray-100 dark:bg-gray-800  border-gray-100 dark:border-gray-900 shadow-sm "
      >
        <div className="mr-32 ml-1 ">
          <TrafficLights className="p-1.5" />
        </div>
        <Button noBorder noPadding className="rounded-r-none mr-[1px]">
          <ChevronLeftIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
        <Button noBorder noPadding className="rounded-l-none">
          <ChevronRightIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
        <div className="w-4"></div>
        <Button variant="selected" noBorder noPadding className="rounded-r-none mr-[1px]">
          <ViewListIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
        <Button noBorder noPadding className="rounded-none mr-[1px]">
          <ViewBoardsIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
        <Button noBorder noPadding className="rounded-l-none">
          <ViewGridIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
        <div className="w-4"></div>
        <div className="relative flex h-7">
          <Input
            placeholder="Search"
            className="placeholder-gray-600 bg-gray-50 text-xs w-32 focus:w-52 transition-all"
          />
          <div className="space-x-1 absolute top-[1px] right-1">
            <Shortcut chars="⌘" />
            <Shortcut chars="S" />
          </div>
        </div>
        <div className="flex-grow"></div>
        <Button noBorder noPadding className="mr-2">
          <CogIcon className="m-0.5 w-4 h-4 dark:text-white" />
        </Button>
      </div>
      <div className="h-[1px] flex-shrink-0 max-w bg-gray-200 dark:bg-gray-700" />
    </>
  );
};
