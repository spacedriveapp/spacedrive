import { CloudIcon } from '@heroicons/react/outline';
import { CogIcon, MenuIcon, PlusIcon } from '@heroicons/react/solid';
import { useBridgeQuery } from '@sd/client';
import { Button } from '@sd/ui';
import byteSize from 'byte-size';
import { DotsSixVertical, Laptop, LineSegments, Plus } from 'phosphor-react';
import React, { useState } from 'react';
import { Device } from '../components/device/Device';
import FileItem from '../components/file/FileItem';
import Dialog from '../components/layout/Dialog';
import { Input } from '../components/primitive';
import { InputContainer } from '../components/primitive/InputContainer';

interface StatItemProps {
  name: string;
  value?: string;
  unit?: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  let size = byteSize(Number(props.value) || 0);
  return (
    <div className="flex flex-col px-4 py-3 duration-75 transform rounded-md cursor-default hover:bg-gray-50 hover:dark:bg-gray-600">
      <span className="text-sm text-gray-400">{props.name}</span>
      <span className="text-2xl font-bold">
        {size.value}
        <span className="ml-1 text-[16px] text-gray-400">{size.unit}</span>
      </span>
    </div>
  );
};

export const OverviewScreen: React.FC<{}> = (props) => {
  const { data: libraryStatistics } = useBridgeQuery('GetLibraryStatistics');
  const { data: clientState } = useBridgeQuery('ClientGetState');

  return (
    <div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll">
      <div data-tauri-drag-region className="flex flex-shrink-0 w-full h-7" />
      <div className="flex flex-col w-full h-screen px-3">
        <div className="flex w-full">
          <div className="flex flex-wrap flex-grow pb-4 space-x-6">
            <StatItem
              name="Total capacity"
              value={libraryStatistics?.total_bytes_capacity}
              unit={libraryStatistics?.total_bytes_capacity}
            />
            <StatItem
              name="Index size"
              value={libraryStatistics?.library_db_size}
              unit={libraryStatistics?.library_db_size}
            />
            <StatItem
              name="Preview media"
              value={libraryStatistics?.preview_media_bytes}
              unit={libraryStatistics?.preview_media_bytes}
            />
            <StatItem
              name="Free space"
              value={libraryStatistics?.total_bytes_free}
              unit={libraryStatistics?.total_bytes_free}
            />
            {/* <StatItem
              name="Total at-risk"
              value={'0'}
              unit={libraryStatistics?.preview_media_bytes}
            />
            <StatItem name="Total backed up" value={'0'} unit={''} /> */}
          </div>
          <div className="space-x-2">
            <Dialog
              title="Add Device"
              description="Connect a new device to your library. Either enter another device's code or copy this one."
              ctaAction={() => {}}
              ctaLabel="Connect"
              trigger={
                <Button
                  size="sm"
                  icon={<PlusIcon className="inline w-4 h-4 -mt-0.5 mr-1" />}
                  variant="gray"
                >
                  Add Device
                </Button>
              }
            >
              <div className="flex flex-col mt-2 space-y-3">
                <div className="flex flex-col">
                  <span className="mb-1 text-xs font-bold uppercase text-gray-450">
                    This Device
                  </span>
                  <Input readOnly disabled value="06ffd64309b24fb09e7c2188963d0207" />
                </div>
                <div className="flex flex-col">
                  <span className="mb-1 text-xs font-bold uppercase text-gray-450">
                    Enter a device code
                  </span>
                  <Input value="" />
                </div>
              </div>
            </Dialog>

            <Button
              size="sm"
              className="w-8"
              noPadding
              icon={<MenuIcon className="inline w-4 h-4" />}
              variant="gray"
            ></Button>
          </div>
        </div>
        {/* <div className="mt-5" /> */}
        <div className="flex flex-col pb-4 space-y-4">
          <Device
            name={clientState?.client_name ?? ''}
            size="1.4TB"
            runningJob={{ amount: 65, task: 'Generating preview media' }}
            locations={[{ name: 'Pictures' }, { name: 'Downloads' }, { name: 'Minecraft' }]}
            type="laptop"
          />
          <Device
            name={`James' iPhone 12`}
            size="47.7GB"
            locations={[{ name: 'Camera Roll' }, { name: 'Notes' }]}
            type="phone"
            removeThisSoon
          />
          <Device
            name={`Spacedrive Server`}
            size="5GB"
            locations={[{ name: 'Cached' }, { name: 'Photos' }, { name: 'Documents' }]}
            type="server"
          />
        </div>
        {/* <hr className="my-4 border-none dark:border-gray-600" /> */}

        {/* <div className="mt-2 space-x-1">
          <FileItem
            selected={selectedFile == 'assets'}
            onClick={() => handleSelect('assets')}
            fileName="assets"
            folder
          />
          <FileItem
            selected={selectedFile == 'tsx'}
            onClick={() => handleSelect('tsx')}
            fileName="App.tsx"
            format="tsx"
            iconName="reactts"
          />
          <FileItem
            selected={selectedFile == 'asc'}
            onClick={() => handleSelect('asc')}
            fileName="asc"
            folder
          />
          <FileItem
            selected={selectedFile == 'scss'}
            onClick={() => handleSelect('scss')}
            fileName="styles.scss"
            format="scss"
            iconName="scss"
          />
          <FileItem
            selected={selectedFile == 'pug'}
            onClick={() => handleSelect('pug')}
            fileName="tailwind.conf.js"
            format="pug"
            iconName="tailwind"
          />
          <FileItem
            selected={selectedFile == 'vite'}
            onClick={() => handleSelect('vite')}
            fileName="vite.config.js"
            format="vite"
            iconName="vite"
          />
          <FileItem
            selected={selectedFile == 'dot'}
            onClick={() => handleSelect('dot')}
            fileName=".prettierrc"
            format="dot"
            iconName="prettier"
          />
          <FileItem
            selected={selectedFile == 'folder'}
            onClick={() => handleSelect('folder')}
            fileName="src"
            folder
          />
          <FileItem
            selected={selectedFile == 'wwcwefwe'}
            onClick={() => handleSelect('wwcwefwe')}
            fileName="index.ts"
            format="ts"
            iconName="typescript"
          />
          <FileItem
            selected={selectedFile == 'werf'}
            onClick={() => handleSelect('werf')}
            fileName="server.ts"
            format="ts"
            iconName="typescript"
          />
          <FileItem
            selected={selectedFile == 'tsex'}
            onClick={() => handleSelect('tsex')}
            fileName="config.json"
            format="json"
            iconName="json"
          />
          <FileItem
            selected={selectedFile == 'tsx3'}
            onClick={() => handleSelect('tsx3')}
            fileName=".vscode"
            folder
          />
          <FileItem
            selected={selectedFile == 'tsx3d'}
            onClick={() => handleSelect('tsx3d')}
            fileName="node_modules"
            folder
          />
        </div>
        <hr className="my-5 border-gray-50 dark:border-gray-600" /> */}

        {/* <hr className="my-5 dark:border-gray-600" /> */}
      </div>
    </div>
  );
};
