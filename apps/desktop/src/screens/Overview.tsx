import React from 'react';

interface StatItemProps {
  name: string;
  value: string;
  unit: string;
}

const StatItem: React.FC<StatItemProps> = (props) => {
  return (
    <div className="flex flex-col p-4 rounded-md dark:bg-gray-800 mt-2">
      <span className="text-gray-400 text-sm">{props.name}</span>
      <span className="font-bold text-2xl">
        {props.value}
        <span className="text-sm text-gray-400 ml-1">{props.unit}</span>
      </span>
    </div>
  );
};

export const OverviewScreen: React.FC<{}> = (props) => {
  return (
    <div className="flex flex-col w-full h-full bg-white dark:bg-gray-900 p-5">
      <h1 className=" font-bold text-xl">Jamie's Library</h1>
      <div className="flex flex-wrap space-x-2 mt-3">
        <StatItem name="Total capacity" value="26.5" unit="TB" />
        <StatItem name="Index size" value="103" unit="MB" />
        <StatItem name="Preview media" value="23.5" unit="GB" />
        <StatItem name="Free space" value="9.2" unit="TB" />

        <StatItem name="Total at-risk" value="1.5" unit="TB" />
        <StatItem name="Total backed up" value="25.3" unit="TB" />
      </div>
    </div>
  );
};
