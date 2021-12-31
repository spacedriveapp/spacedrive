import React from 'react';
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
    <div className="flex flex-col p-4 mt-2 rounded-md bg-gray-50 dark:bg-gray-650">
      <span className="text-sm text-gray-400">{props.name}</span>
      <span className="text-2xl font-bold">
        {props.value}
        <span className="ml-1 text-sm text-gray-400">{props.unit}</span>
      </span>
    </div>
  );
};

export const OverviewScreen: React.FC<{}> = (props) => {
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
      <div className="-mt-[1px] space-x-2 ml-1">
        <Tag color="red">Videos</Tag>
        <Tag color="orange">DSLR Photos</Tag>
        <Tag color="yellow">Camera Roll</Tag>
        <Tag color="green">NFTs</Tag>
        <Tag color="pink">Screenshots</Tag>
        <Tag color="blue">Documents</Tag>
        <Tag color="purple">Repositories</Tag>
      </div>
      <div className="mt-8 space-x-1">
        <FileItem fileName="hello.tsx" format="tsx" iconName="reactts" />
        <FileItem fileName="styles.scss" format="scss" iconName="scss" />
        <FileItem fileName="yes.pug" format="pug" iconName="pug" />
        <FileItem fileName="vite.config.js" format="vite" iconName="vite" />
        <FileItem fileName=".prettierrc" format="dot" iconName="prettier" />
        <FileItem fileName="index.ts" format="ts" iconName="typescript" />
        <FileItem fileName="server.ts" format="ts" iconName="typescript" />
        <FileItem fileName="config.json" format="json" iconName="json" />
      </div>
    </div>
  );
};
