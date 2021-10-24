import React from 'react';
import { useExplorerStore, useSelectedFile } from '../../store/explorer';
import { Transition } from '@headlessui/react';
import { IFile } from '../../types';
import { useAppState } from '../../store/app';
import { convertFileSrc } from '@tauri-apps/api/tauri';

interface MetaItemProps {
  title: string;
  value: string;
}

const MetaItem = (props: MetaItemProps) => {
  return (
    <div className="meta-item flex flex-col p-3">
      <h5 className="font-bold text-sm">{props.title}</h5>
      <p className="break-all text-xs text-gray-300">{props.value}</p>
    </div>
  );
};

const Divider = () => <div className="w-full my-1 h-[1px] bg-gray-700" />;

export const Inspector = () => {
  const selectedFile = useSelectedFile();

  const isOpen = !!selectedFile;

  const file = selectedFile;

  return (
    <Transition
      show={isOpen}
      enter="transition-translate ease-in-out duration-200"
      enterFrom="translate-x-64"
      enterTo="translate-x-0"
      leave="transition-translate ease-in-out duration-200"
      leaveFrom="translate-x-0"
      leaveTo="translate-x-64"
    >
      <div className="h-full w-60  right-0 top-0 m-2">
        <div className="flex flex-col overflow-hidden h-full rounded-lg bg-gray-700 shadow-lg  select-text">
          <div className="h-32 bg-gray-750 w-full flex justify-center items-center">
            <img
              src={convertFileSrc(
                `${useAppState.getState().file_type_thumb_dir}/${
                  file?.is_dir ? 'folder' : file?.extension
                }.png`
              )}
              className="h-24"
            />
          </div>
          <h3 className="font-bold p-3 text-base">{file?.name}</h3>
          <MetaItem title="Checksum" value={file?.meta_checksum as string} />
          <Divider />
          <MetaItem title="Uri" value={file?.uri as string} />
        </div>
      </div>
    </Transition>
  );
};
