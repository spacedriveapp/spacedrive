import * as DropdownMenuPrimitive from '@radix-ui/react-dropdown-menu';
import {
  CaretRightIcon,
  CheckIcon,
  CropIcon,
  EyeClosedIcon,
  EyeOpenIcon,
  FileIcon,
  FrameIcon,
  GridIcon,
  Link2Icon,
  MixerHorizontalIcon,
  PersonIcon,
  TransparencyGridIcon
} from '@radix-ui/react-icons';
import cx from 'clsx';
import React, { ReactNode, useState } from 'react';
import { Button } from './Button';

interface RadixMenuItem {
  label: string;
  shortcut?: string;
  icon?: ReactNode;
}

interface User {
  name: string;
  url?: string;
}

const generalMenuItems: RadixMenuItem[] = [
  {
    label: 'New File',
    icon: <FileIcon className="mr-2 h-3.5 w-3.5" />,
    shortcut: '⌘+N'
  },
  {
    label: 'Settings',
    icon: <MixerHorizontalIcon className="mr-2 h-3.5 w-3.5" />,
    shortcut: '⌘+,'
  }
];

const regionToolMenuItems: RadixMenuItem[] = [
  {
    label: 'Frame',
    icon: <FrameIcon className="mr-2 h-3.5 w-3.5" />,
    shortcut: '⌘+F'
  },
  {
    label: 'Crop',
    icon: <CropIcon className="mr-2 h-3.5 w-3.5" />,
    shortcut: '⌘+S'
  }
];

const users: User[] = [
  {
    name: 'Adam',
    url: 'https://github.com/adamwathan.png'
  },
  {
    name: 'Steve',
    url: 'https://github.com/steveschoger.png'
  },
  {
    name: 'Robin',
    url: 'https://github.com/robinmalfait.png'
  }
];

interface Props {}

const DropdownMenu = (props: Props) => {
  const [showGrid, setShowGrid] = useState(false);
  const [showUi, setShowUi] = useState(false);

  return (
    <div className="relative inline-block text-left">
      <DropdownMenuPrimitive.Root>
        <DropdownMenuPrimitive.Trigger asChild>
          <Button>Click</Button>
        </DropdownMenuPrimitive.Trigger>

        <DropdownMenuPrimitive.Content
          align="end"
          sideOffset={5}
          className={cx(
            ' radix-side-top:animate-slide-up radix-side-bottom:animate-slide-down',
            'w-48 rounded-lg px-1.5 py-1 shadow-md md:w-56',
            'bg-white dark:bg-gray-800'
          )}
        >
          {generalMenuItems.map(({ label, icon, shortcut }, i) => (
            <DropdownMenuPrimitive.Item
              key={`${label}-${i}`}
              className={cx(
                'flex cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none',
                'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
              )}
            >
              {icon}
              <span className="flex-grow text-gray-700 dark:text-gray-300">{label}</span>
              {shortcut && <span className="text-xs">{shortcut}</span>}
            </DropdownMenuPrimitive.Item>
          ))}

          <DropdownMenuPrimitive.Separator className="h-px my-1 bg-gray-200 dark:bg-gray-700" />

          <DropdownMenuPrimitive.CheckboxItem
            checked={showGrid}
            onCheckedChange={setShowGrid}
            className={cx(
              'flex w-full cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none',
              'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
            )}
          >
            {showGrid ? (
              <GridIcon className="w-4 h-4 mr-2" />
            ) : (
              <TransparencyGridIcon className="mr-2 h-3.5 w-3.5 text-gray-700 dark:text-gray-300" />
            )}
            <span className="flex-grow text-gray-700 dark:text-gray-300">Show Grid</span>
            <DropdownMenuPrimitive.ItemIndicator>
              <CheckIcon className="h-3.5 w-3.5" />
            </DropdownMenuPrimitive.ItemIndicator>
          </DropdownMenuPrimitive.CheckboxItem>

          <DropdownMenuPrimitive.CheckboxItem
            checked={showUi}
            onCheckedChange={setShowUi}
            className={cx(
              'flex w-full cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none',
              'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
            )}
          >
            {showUi ? (
              <EyeOpenIcon className="mr-2 h-3.5 w-3.5" />
            ) : (
              <EyeClosedIcon className="mr-2 h-3.5 w-3.5" />
            )}
            <span className="flex-grow text-gray-700 dark:text-gray-300">Show UI</span>
            <DropdownMenuPrimitive.ItemIndicator>
              <CheckIcon className="h-3.5 w-3.5" />
            </DropdownMenuPrimitive.ItemIndicator>
          </DropdownMenuPrimitive.CheckboxItem>

          <DropdownMenuPrimitive.Separator className="h-px my-1 bg-gray-200 dark:bg-gray-700" />

          <DropdownMenuPrimitive.Label className="px-2 py-2 text-xs text-gray-700 select-none dark:text-gray-200">
            Region Tools
          </DropdownMenuPrimitive.Label>

          {regionToolMenuItems.map(({ label, icon, shortcut }, i) => (
            <DropdownMenuPrimitive.Item
              key={`${label}-${i}`}
              className={cx(
                'flex cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none',
                'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
              )}
            >
              {icon}
              <span className="flex-grow text-gray-700 dark:text-gray-300">{label}</span>
              {shortcut && <span className="text-xs">{shortcut}</span>}
            </DropdownMenuPrimitive.Item>
          ))}

          <DropdownMenuPrimitive.Separator className="h-px my-1 bg-gray-200 dark:bg-gray-700" />

          <DropdownMenuPrimitive.Root>
            <DropdownMenuPrimitive.TriggerItem
              className={cx(
                'flex w-full cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none',
                'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
              )}
            >
              <Link2Icon className="mr-2 h-3.5 w-3.5" />
              <span className="flex-grow text-gray-700 dark:text-gray-300">Share</span>
              <CaretRightIcon className="h-3.5 w-3.5" />
            </DropdownMenuPrimitive.TriggerItem>
            <DropdownMenuPrimitive.Content
              className={cx(
                'origin-radix-dropdown-menu radix-side-right:animate-scale-in',
                'w-full rounded-md px-1 py-1 text-xs shadow-md',
                'bg-white dark:bg-gray-800'
              )}
            >
              {users.map(({ name, url }, i) => (
                <DropdownMenuPrimitive.Item
                  key={`${name}-${i}`}
                  className={cx(
                    'flex w-28 cursor-default select-none items-center rounded-md px-2 py-2 text-xs outline-none md:w-32',
                    'text-gray-400 focus:bg-gray-50 dark:text-gray-500 dark:focus:bg-gray-900'
                  )}
                >
                  {url ? (
                    <img className="mr-2.5 h-6 w-6 rounded-full" src={url} />
                  ) : (
                    <PersonIcon className="mr-2.5 h-6 w-6" />
                  )}
                  <span className="text-gray-700 dark:text-gray-300">{name}</span>
                </DropdownMenuPrimitive.Item>
              ))}
            </DropdownMenuPrimitive.Content>
          </DropdownMenuPrimitive.Root>
        </DropdownMenuPrimitive.Content>
      </DropdownMenuPrimitive.Root>
    </div>
  );
};

export default DropdownMenu;
