import { CogIcon, CollectionIcon, CubeTransparentIcon, DatabaseIcon } from '@heroicons/react/solid';
import { GearSix } from 'phosphor-react';
import React, { SetStateAction, useState } from 'react';
import { Button, ButtonVariant } from '../primative';
import { Dropdown } from '../primative/Dropdown';
import { DefaultProps } from '../primative/types';

const tabs = [
  { name: 'Drive', icon: CollectionIcon },
  { name: 'Storage', icon: DatabaseIcon },
  { name: 'Explorer', icon: CubeTransparentIcon },
  { name: 'Settings', icon: CogIcon }
];

interface SidebarButtonProps extends DefaultProps {
  name: string;
  icon: any;
  setActiveTab: (name: string) => any;
  getVarient: (name: string) => ButtonVariant;
}
interface SidebarProps extends DefaultProps {}

const SidebarButton: React.FC<SidebarButtonProps> = (props) => {
  return (
    <Button
      onClick={() => props.setActiveTab(props.name)}
      variant={props.getVarient(props.name)}
      justifyLeft
      noBorder
      className="items-center shadow-none text-gray-500 hover:text-gray-500 cursor-default"
      size="sm"
    >
      {props.icon && <props.icon className="w-4 h-4 mr-2" />}
      {props.name}
    </Button>
  );
};

export const Sidebar: React.FC<SidebarProps> = (props) => {
  const [activeTab, setActiveTab] = useState(tabs[0].name);

  const getVarient = (name: string) => (activeTab == name ? 'selected' : 'default');

  return (
    <div className="w-48 flex flex-col flex-wrap flex-shrink-0 min-h-full bg-gray-50 dark:bg-gray-800 border-gray-100 border-r dark:border-gray-700 px-2  space-y-0.5">
      <Dropdown
        buttonProps={{
          justifyLeft: true,
          className: 'mb-1 flex-shrink-0 w-175px]',
          variant: 'gray'
        }}
        buttonText="Jamie's Library"
        items={[[{ name: `Jamie's Library` }, { name: 'Subto' }], [{ name: 'Add Library' }]]}
      />

      {tabs.map((button, index) => (
        <SidebarButton
          name={button.name}
          icon={button.icon}
          key={index}
          getVarient={getVarient}
          setActiveTab={setActiveTab}
        />
      ))}
    </div>
  );
};
