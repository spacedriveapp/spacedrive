import { Transition } from '@headlessui/react';
import clsx from 'clsx';
import React from 'react';
import { XIcon } from '@heroicons/react/solid';
import { Button } from '../primitive';
import { useNavigate } from 'react-router-dom';
import { MacOSTrafficLights } from '../file/Sidebar';

export interface ModalProps {
  full?: boolean;
}

export const Modal: React.FC<ModalProps> = (props) => {
  const navigate = useNavigate();
  return (
    <div
      className={clsx('absolute w-screen h-screen z-30', { 'pointer-events-none hidden': !open })}
    >
      <MacOSTrafficLights />
      <div className="flex w-screen h-screen p-2 pt-12">
        <Transition
          show
          enter="transition duration-150"
          enterFrom="opacity-0"
          enterTo="opacity-100"
          leave="transition duration-200"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <div
            data-tauri-drag-region
            onClick={() => navigate('/')}
            className="absolute -z-50 w-screen h-screen left-0 top-0 rounded-2xl bg-white dark:bg-gray-800 bg-opacity-90"
          />
        </Transition>
        <Button
          onClick={() => navigate('/')}
          variant="gray"
          className="!px-1.5 absolute top-2 right-2"
        >
          <XIcon className="w-4 h-4" />
        </Button>
        <Transition
          show
          className="flex flex-grow"
          appear
          enter="transition ease-in-out-back duration-200"
          enterFrom="opacity-0 translate-y-5"
          enterTo="opacity-100"
          leave="transition duration-200"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <div className="flex flex-grow max-w-7xl mx-auto z-30 shadow-2xl bg-white rounded-lg shadow-xl dark:bg-gray-650 ">
            {props.children}
          </div>
        </Transition>
      </div>
    </div>
  );
};
