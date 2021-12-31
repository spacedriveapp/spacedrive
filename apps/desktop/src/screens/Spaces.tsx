import React from 'react';
import FileItem from '../components/file/FileItem';

export const SpacesScreen: React.FC<{}> = (props) => {
  return (
    <div className="flex flex-col w-full h-full px-2 py-5">
      <div className="space-x-1">
        <FileItem fileName="hello.tsx" format="tsx" iconName="reactts" />
        <FileItem fileName="styles.scss" format="scss" iconName="scss" />
        <FileItem fileName="yes.pug" format="pug" iconName="pug" />
        <FileItem fileName="vite.config.js" format="vite" iconName="vite" />
        <FileItem fileName=".prettierrc" format="dot" iconName="prettier" />
        <FileItem fileName="index.ts" format="ts" iconName="typescript" />
        <FileItem fileName="server.ts" format="ts" iconName="typescript" />
        <FileItem fileName="config.json" format="json" iconName="json" />
      </div>
      {/* <div className="flex flex-wrap p-2 my-3 space-x-2 bg-black rounded">
        <div className="w-10 h-10 rounded bg-gray-950"/>
        <div className="w-10 h-10 bg-gray-900 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-850"/>
        <div className="w-10 h-10 bg-gray-800 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-750"/>
        <div className="w-10 h-10 bg-gray-700 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-650"/>
        <div className="w-10 h-10 bg-gray-600 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-550"/>
        <div className="w-10 h-10 bg-gray-400 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-450"/>
        <div className="w-10 h-10 bg-gray-400 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-350"/>
        <div className="w-10 h-10 bg-gray-300 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-250"/>
        <div className="w-10 h-10 bg-gray-200 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-150"/>
        <div className="w-10 h-10 bg-gray-100 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-50"/>
      </div>*/}
    </div>
  );
};
