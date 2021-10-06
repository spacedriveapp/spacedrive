import React from 'react';
import { Menu, Transition } from '@headlessui/react';
import { Fragment, useEffect, useRef, useState } from 'react';
import { ChevronDownIcon } from '@heroicons/react/solid';
import { DefaultOptions } from '@apollo/client';
import { Button, ButtonProps } from '.';
import clsx from 'clsx';

type Section = {
  name: string;
  icon?: any;
  selected?: boolean;
}[];

export interface DropdownProps extends DefaultOptions {
  items: Section[];
  buttonText: string;
  buttonProps: ButtonProps;
}

export const Dropdown: React.FC<DropdownProps> = (props) => {
  return (
    <div className="flex mt-2">
      <Menu as="div" className="relative inline-block text-left">
        <div>
          <Menu.Button className="outline-none">
            <Button size="sm" {...props.buttonProps}>
              {props.buttonText}
              <div className="flex-grow" />
              <ChevronDownIcon
                className="w-5 h-5 ml-2 -mr-1 text-violet-200 hover:text-violet-100 "
                aria-hidden="true"
              />
            </Button>
          </Menu.Button>
        </div>
        <Transition
          as={Fragment}
          enter="transition ease-out duration-100"
          enterFrom="transform opacity-0 scale-95"
          enterTo="transform opacity-100 scale-100"
          leave="transition ease-in duration-75"
          leaveFrom="transform opacity-100 scale-100"
          leaveTo="transform opacity-0 scale-95"
        >
          <Menu.Items className="absolute left-0 w-40 mt-1 origin-top-left bg-white dark:bg-gray-900 divide-y divide-gray-100 dark:divide-gray-700 rounded-md shadow-lg ring-1 ring-black ring-opacity-5 focus:outline-none">
            {props.items.map((item, index) => (
              <div key={index} className="px-1 py-1">
                {item.map((button, index) => (
                  <Menu.Item key={index}>
                    {({ active }) => (
                      <button
                        className={clsx(
                          'text-sm group flex rounded-md items-center w-full px-2 py-1',
                          {
                            'bg-primary text-white': active,
                            'text-gray-900 dark:text-gray-200': !active
                          }
                        )}
                      >
                        {button.icon && (
                          <button.icon
                            className={clsx('mr-2 w-4 h-4', {
                              'dark:text-gray-100': active,
                              'text-gray-600 dark:text-gray-200': !active
                            })}
                          />
                        )}
                        {button.name}
                      </button>
                    )}
                  </Menu.Item>
                ))}
              </div>
            ))}
          </Menu.Items>
        </Transition>
      </Menu>
    </div>
  );
};
