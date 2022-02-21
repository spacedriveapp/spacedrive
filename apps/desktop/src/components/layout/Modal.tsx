import { Transition } from '@headlessui/react';
import clsx from 'clsx';
import React, {ComponentProps, createContext, useState} from 'react';
import {atom, useAtom, WritableAtom} from "jotai";
import {createPortal} from "react-dom";
export interface ModalProps {
  name: string;
  open: WritableAtom<boolean, boolean>,
  full?: boolean;
}

export function createModal(name: string): ModalProps {
  return { name, open: atom(true) as WritableAtom<boolean, boolean> }
}

export const Modal: React.FC<ModalProps> = (props) => {
  const [open, setOpen] = useAtom(props.open);

  return (
    <div
      data-tauri-drag-region
      onClick={() => setOpen(false)}
      className={clsx(
        'transition-opacity absolute flex w-full h-full p-5 t-0 bg-black bg-opacity-80 m-[1px] rounded-lg z-50',
        { 'pointer-events-none hidden': !open }
      )}
    >
      <Transition
        show={open}
        // enter="transition-translate	ease-in-out duration-200"
        // enterFrom="-scale-2"
        // enterTo="scale-0"
        // leave="transition-translate ease-in-out duration-200"
        // leaveFrom="scale-0"
        // leaveTo="-scale-2"
      >
        <div className="w-full h-full flex flex-grow bg-white rounded-lg shadow-xl dark:bg-gray-850">
          {props.children}
        </div>
      </Transition>
    </div>);
};
