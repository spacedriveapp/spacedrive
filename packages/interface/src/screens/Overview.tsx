import React, { useState } from 'react';
import FileItem from '../components/file/FileItem';

interface StatItemProps {
  name: string;
  value: string;
  unit: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  return (
    <div className="flex flex-col p-4 mt-2 duration-75 transform rounded-md cursor-default hover:bg-gray-50 hover:dark:bg-gray-600">
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
    // if (selectedFile === key) setSelectedFile(null);
    // else setSelectedFile(key);
    setSelectedFile(key);
  }

  return (
    <div className="flex flex-col w-full h-screen">
      <div data-tauri-drag-region className="flex flex-shrink-0 w-full h-7" />
      <div className="flex flex-col w-full h-screen px-5 pb-3 overflow-scroll">
        <div className="flex items-center justify-center w-full">
          <div className="flex space-x-2">
            <StatItem name="Total capacity" value="26.5" unit="TB" />
            <StatItem name="Index size" value="103" unit="MB" />
            <StatItem name="Preview media" value="23.5" unit="GB" />
          </div>
          <div className="flex flex-col items-center w-56">
            {/* <img
              alt="spacedrive-logo"
              src="/images/spacedrive_logo.png"
              className="pointer-events-none w-28 h-28"
            /> */}
            {/* <span className="text-lg font-bold heading-1">Spacedrive</span>
          <span className="mt-0.5 text-sm text-gray-400 mb-5">v1.0.11 (pre-alpha)</span> */}
            {/* <span className="font-bold text-gray-400 text-md heading-1">Jamie's Library</span>
          <span className="mt-1 text-xs text-gray-500 ">lib-71230e11c869</span> */}
          </div>
          <div className="flex space-x-2">
            <StatItem name="Free space" value="9.2" unit="TB" />
            <StatItem name="Total at-risk" value="1.5" unit="TB" />
            <StatItem name="Total backed up" value="25.3" unit="TB" />
          </div>
        </div>

        <hr className="my-5 border-gray-50 dark:border-gray-600" />
        <div className="mt-2 space-x-1">
          <FileItem
            selected={selectedFile == 'tsx'}
            onClick={() => handleSelect('tsx')}
            fileName="App.tsx"
            format="tsx"
            iconName="reactts"
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
            fileName="cool.pug"
            format="pug"
            iconName="pug"
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
        <hr className="my-5 border-gray-50 dark:border-gray-600" />
        <p className="px-5 py-3 mb-3 text-gray-400 rounded-md bg-gray-50 dark:text-gray-500 dark:bg-gray-600">
          <b>Note: </b>This is a pre-alpha build of Spacedrive, an open source personal cloud
          powered by your daily devices. Under the hood, a secure Rust based virtual filesystem
          synchronized cross-platform in realtime. Enjoy this barely functional UI while pre-alpha
          is still in progress.
        </p>
        {/* <hr className="my-5 dark:border-gray-600" /> */}
      </div>
    </div>
  );
};
