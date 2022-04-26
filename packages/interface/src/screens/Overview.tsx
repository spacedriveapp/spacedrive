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
        <span className="ml-1 text-sm text-gray-400">{props.unit}</span>
      </span>
    </div>
  );
};

export const OverviewScreen: React.FC<{}> = (props) => {
  const [selectedFile, setSelectedFile] = useState<null | string>(null);

  function handleSelect(key: string) {
    if (selectedFile === key) setSelectedFile(null);
    else setSelectedFile(key);
  }

  return (
    <div className="flex flex-col w-full h-screen">
      <div data-tauri-drag-region className="flex flex-shrink-0 w-full h-7" />
      <div className="flex flex-col w-full h-screen px-3 overflow-scroll">
        <div className="flex items-center w-full ml-4">
          <div className="flex flex-wrap p-2 space-x-6">
            <StatItem name="Total capacity" value="26.5" unit="TB" />
            <StatItem name="Index size" value="103" unit="MB" />
            <StatItem name="Preview media" value="23.5" unit="GB" />
            <StatItem name="Free space" value="9.2" unit="TB" />
            <StatItem name="Total at-risk" value="1.5" unit="TB" />
            <StatItem name="Total backed up" value="25.3" unit="TB" />
          </div>
        </div>
        <div className="mt-5" />
        <div className="flex flex-col space-y-4">
          <Device
            name="Spacedad"
            size="1.4TB"
            runningJob={{ amount: 65, task: 'Generating preview media' }}
            locations={[{ name: 'Pictures' }, { name: 'Downloads' }, { name: 'Minecraft' }]}
            type="laptop"
          />
          <Device
            name="Jamies iPhone"
            size="47.7GB"
            locations={[{ name: 'Camera Roll' }, { name: 'Notes' }]}
            type="phone"
          />
        </div>
        {/* <hr className="my-5 border-gray-50 dark:border-gray-600" /> */}

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
