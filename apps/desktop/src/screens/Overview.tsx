import React, { useState } from 'react';
import ReactJson from 'react-json-view';
import FileItem from '../components/file/FileItem';
import { useAppState } from '../store/global';

interface StatItemProps {
  name: string;
  value: string;
  unit: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  return (
    <div className="flex flex-col p-4 mt-2 rounded-md bg-gray-50 dark:bg-gray-550">
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
  const app = useAppState();

  function handleSelect(key: string) {
    // if (selectedFile === key) setSelectedFile(null);
    // else setSelectedFile(key);
    setSelectedFile(key);
  }

  return (
    <div className="flex flex-col w-full h-screen px-5 py-3 overflow-scroll">
      <div className="flex justify-center w-full">
        <div className="flex flex-col items-center">
          <img
            alt="spacedrive-logo"
            src="/images/spacedrive_logo.png"
            className="w-24 h-24 mt-2 pointer-events-none"
          />
          <span className="text-lg font-bold heading-1">Spacedrive</span>
          <span className="mt-0.5 text-sm text-gray-400 mb-5">v1.0.11 (pre-alpha)</span>
        </div>
      </div>
      <hr className="my-5 dark:border-gray-600" />
      <div className="flex flex-wrap space-x-2">
        <StatItem name="Total capacity" value="26.5" unit="TB" />
        <StatItem name="Index size" value="103" unit="MB" />
        <StatItem name="Preview media" value="23.5" unit="GB" />
        <StatItem name="Free space" value="9.2" unit="TB" />
        <StatItem name="Total at-risk" value="1.5" unit="TB" />
        <StatItem name="Total backed up" value="25.3" unit="TB" />
      </div>
      <hr className="my-5 dark:border-gray-600" />
      <div className="mt-2 -ml-3 space-x-1">
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
      <hr className="my-5 dark:border-gray-600" />
      <div className="mt-2 mb-24 select-text">
        <ReactJson
          // collapsed
          enableClipboard={false}
          displayDataTypes={false}
          theme="ocean"
          src={app.config}
          style={{
            padding: 20,
            borderRadius: 5,
            backgroundColor: '#101016',
            border: 1,
            borderColor: '#1E1E27',
            borderStyle: 'solid'
          }}
        />
      </div>
    </div>
  );
};
