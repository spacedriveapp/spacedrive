import React, { useState } from 'react';
import FileItem from '../components/file/FileItem';
import { Button } from '../components/primitive';
import { Tag } from '../components/primitive/Tag';

interface StatItemProps {
  name: string;
  value: string;
  unit: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  return (
    <div className="flex flex-col p-4 mt-2 rounded-md shadow-md bg-gray-50 dark:bg-gray-600">
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
    <div className="flex flex-col w-full h-full px-5 py-3">
      <div className="flex flex-wrap space-x-2">
        <StatItem name="Total capacity" value="26.5" unit="TB" />
        <StatItem name="Index size" value="103" unit="MB" />
        <StatItem name="Preview media" value="23.5" unit="GB" />
        <StatItem name="Free space" value="9.2" unit="TB" />
        <StatItem name="Total at-risk" value="1.5" unit="TB" />
        <StatItem name="Total backed up" value="25.3" unit="TB" />
      </div>
      <hr className="my-5 dark:border-gray-600" />

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
    </div>
  );
};
