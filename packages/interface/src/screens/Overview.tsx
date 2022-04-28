import { useBridgeQuery } from '@sd/client';
import byteSize from 'byte-size';
import { DotsSixVertical, Laptop, LineSegments } from 'phosphor-react';
import React, { useState } from 'react';
import { Device } from '../components/device/Device';
import FileItem from '../components/file/FileItem';

interface StatItemProps {
  name: string;
  value: string;
  unit: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  return (
    <div className="flex flex-col px-4 py-3 duration-75 transform rounded-md cursor-default hover:bg-gray-50 hover:dark:bg-gray-600">
      <span className="text-sm text-gray-400">{props.name}</span>
      <span className="text-2xl font-bold">
        {props.value}
        <span className="ml-1 text-[16px] text-gray-400">{props.unit}</span>
      </span>
    </div>
  );
};

export const OverviewScreen: React.FC<{}> = (props) => {
  const { data: libraryStatistics } = useBridgeQuery('GetLibraryStatistics');

  return (
    <div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll">
      <div data-tauri-drag-region className="flex flex-shrink-0 w-full h-7" />
      <div className="flex flex-col w-full h-screen px-3">
        <div className="flex items-center w-full">
          <div className="flex flex-wrap pb-4 space-x-6">
            <StatItem
              name="Total capacity"
              value={byteSize(Number(libraryStatistics?.total_bytes_capacity)).value || '0'}
              unit={byteSize(Number(libraryStatistics?.total_bytes_capacity)).unit}
            />
            <StatItem
              name="Index size"
              value={byteSize(Number(libraryStatistics?.library_db_size)).value || '0'}
              unit={byteSize(Number(libraryStatistics?.library_db_size)).unit}
            />
            <StatItem
              name="Preview media"
              value={byteSize(Number(libraryStatistics?.preview_media_bytes)).value || '0'}
              unit={byteSize(Number(libraryStatistics?.preview_media_bytes)).unit}
            />
            <StatItem
              name="Free space"
              value={byteSize(Number(libraryStatistics?.total_bytes_free)).value || '0'}
              unit={byteSize(Number(libraryStatistics?.total_bytes_free)).unit}
            />
            <StatItem
              name="Total at-risk"
              value={'0'}
              unit={byteSize(Number(libraryStatistics?.preview_media_bytes)).unit}
            />
            <StatItem
              name="Total backed up"
              value={byteSize(Number(libraryStatistics?.preview_media_bytes)).value || '0'}
              unit={byteSize(Number(libraryStatistics?.preview_media_bytes)).unit}
            />
          </div>
        </div>
        {/* <div className="mt-5" /> */}
        <div className="flex flex-col pb-4 space-y-4">
          <Device
            name="James' MBP"
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
