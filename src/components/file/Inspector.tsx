import React from 'react';
import { useExplorerStore, useSelectedFile } from '../../store/explorer';
import { Transition } from '@headlessui/react';
import { IFile } from '../../types';
import { useAppState } from '../../store/app';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import moment from 'moment';
import { Button } from '../primative';
import { ShareIcon } from '@heroicons/react/solid';
import { Heart, Link } from 'phosphor-react';

interface MetaItemProps {
  title: string;
  value: string;
}

const MetaItem = (props: MetaItemProps) => {
  return (
    <div className="meta-item flex flex-col px-3 py-1">
      <h5 className="font-bold text-xs">{props.title}</h5>
      <p className="break-all text-xs text-gray-600 dark:text-gray-300 truncate">{props.value}</p>
    </div>
  );
};

const Divider = () => <div className="w-full my-1 h-[1px] bg-gray-100 dark:bg-gray-600" />;

export const Inspector = () => {
  const selectedFile = useSelectedFile();

  const isOpen = !!selectedFile;

  const file = selectedFile;

  return (
    <Transition
      show={true}
      enter="transition-translate ease-in-out duration-200"
      enterFrom="translate-x-64"
      enterTo="translate-x-0"
      leave="transition-translate ease-in-out duration-200"
      leaveFrom="translate-x-0"
      leaveTo="translate-x-64"
    >
      <div className="h-full w-60 right-0 top-0 m-2 border border-gray-100 dark:border-gray-850 rounded-lg ">
        {!!file && (
          <div className="flex flex-col overflow-hidden h-full rounded-lg bg-white dark:bg-gray-700 select-text">
            <div className="h-32 bg-gray-50 dark:bg-gray-900 rounded-t-lg w-full flex justify-center items-center">
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
            <div className="flex flex-row m-3 space-x-2">
              <Button size="sm">
                <Heart className="w-4 h-4" />
              </Button>
              <Button size="sm">
                <ShareIcon className="w-4 h-4" />
              </Button>
              <Button size="sm">
                <Link className="w-4 h-4" />
              </Button>
            </div>
            <MetaItem title="Checksum" value={file?.meta_checksum as string} />
            <Divider />
            <MetaItem title="Uri" value={file?.uri as string} />
            <Divider />
            <MetaItem
              title="Date Created"
              value={moment(file?.date_created).format('MMMM Do YYYY, h:mm:ss a')}
            />
            {/* <div className="flex flex-row m-3">
              <Button size="sm">Mint</Button>
            </div> */}
            {/* <MetaItem title="Date Last Modified" value={file?.date_modified} />
            <MetaItem title="Date Indexed" value={file?.date_indexed} /> */}
          </div>
        )}
      </div>
    </Transition>
  );
};
