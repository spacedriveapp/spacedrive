import React from 'react';
import { useExplorerStore, useSelectedFile } from '../../store/explorer';
import { Transition } from '@headlessui/react';
import { IFile } from '../../types';
import { useAppState } from '../../store/global';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import moment from 'moment';
import { Button } from '../primitive';
import { ShareIcon } from '@heroicons/react/solid';
import { Heart, Link } from 'phosphor-react';

interface MetaItemProps {
  title: string;
  value: string;
}

const MetaItem = (props: MetaItemProps) => {
  return (
    <div className="flex flex-col px-3 py-1 meta-item">
      <h5 className="text-xs font-bold">{props.title}</h5>
      <p className="text-xs text-gray-600 break-all truncate dark:text-gray-300">{props.value}</p>
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
      <div className="top-0 right-0 h-full m-2 border border-gray-100 rounded-lg w-60 dark:border-gray-850 ">
        {!!file && (
          <div className="flex flex-col h-full overflow-hidden bg-white rounded-lg select-text dark:bg-gray-700">
            <div className="flex items-center justify-center w-full h-32 rounded-t-lg bg-gray-50 dark:bg-gray-900">
              <img
                src={convertFileSrc(
                  `${useAppState.getState().config.file_type_thumb_dir}/${
                    file?.is_dir ? 'folder' : file?.extension
                  }.png`
                )}
                className="h-24"
              />
            </div>
            <h3 className="p-3 text-base font-bold">{file?.name}</h3>
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
            <MetaItem title="Checksum" value={file?.meta_integrity_hash as string} />
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
