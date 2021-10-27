import { Transition } from '@headlessui/react';
import clsx from 'clsx';
import React, { createContext, useState } from 'react';

export interface ModalProps {}

const modalContext = createContext({ open: false });

export const Modal = (props: ModalProps) => {
  const [open, setOpen] = useState(false);
  return (
    <div
      data-tauri-drag-region
      onClick={() => setOpen(false)}
      className={clsx(
        'transition-opacity w-screen h-screen p-5 absolute t-0 bg-black bg-opacity-30 m-[1px] rounded-lg',
        { 'pointer-events-none hidden': !open }
      )}
    >
      <Transition
        show={open}
        enter="transition-translate	ease-in-out duration-200"
        enterFrom="-scale-2"
        enterTo="scale-0"
        leave="transition-translate ease-in-out duration-200"
        leaveFrom="scale-0"
        leaveTo="-scale-2"
      >
        <div className="w-full h-full bg-white dark:bg-gray-850 rounded-lg shadow-xl">
          <h1 className="m-10">hi</h1>
        </div>
      </Transition>
    </div>
  );
};
