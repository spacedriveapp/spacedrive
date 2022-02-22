import {
  CloudIcon,
  CogIcon,
  KeyIcon,
  LockClosedIcon,
  PhotographIcon,
  TagIcon,
  UsersIcon
} from '@heroicons/react/solid';
import React from 'react';
// import { dummyIFile, FileList } from '../components/file/FileList';
import { SidebarLink } from '../components/file/Sidebar';
import { HardDrive, PaintBrush } from 'phosphor-react';
import clsx from 'clsx';
import { BrowserRouter as Router, Route, Switch } from 'react-router-dom';
import GeneralSettings from './settings/General';

//@ts-ignore
// import { Spline } from 'react-spline';
// import WINDOWS_SCENE from '../assets/spline/scene.json';

const Icon = ({ component: Icon, ...props }: any) => (
  <Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{ className?: string }> = ({ children, className }) => (
  <div className={clsx('mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300', className)}>
    {children}
  </div>
);

export const SettingsScreen: React.FC<{}> = () => {
  return (
    <Router>
      <div className="flex flex-row w-full">
        <div className="p-8 w-60 h-full border-r border-gray-550">
          <Heading className="mt-0">Client</Heading>
          <SidebarLink to="/general">
            <Icon component={CogIcon} />
            General
          </SidebarLink>
          <SidebarLink to="/security">
            <Icon component={LockClosedIcon} />
            Security
          </SidebarLink>

          <SidebarLink to="/appearance">
            <Icon component={PaintBrush} />
            Appearance
          </SidebarLink>
          <Heading>Library</Heading>

          <SidebarLink to="/locations">
            <Icon component={HardDrive} />
            Locations
          </SidebarLink>
          <SidebarLink to="/media">
            <Icon component={PhotographIcon} />
            Media
          </SidebarLink>
          <SidebarLink to="/keys">
            <Icon component={KeyIcon} />
            Keys
          </SidebarLink>
          <SidebarLink to="/tags">
            <Icon component={TagIcon} />
            Tags
          </SidebarLink>

          <Heading>Cloud</Heading>
          <SidebarLink to="/sync">
            <Icon component={CloudIcon} />
            Sync
          </SidebarLink>
          <SidebarLink to="/contacts">
            <Icon component={UsersIcon} />
            Contacts
          </SidebarLink>
        </div>
        <div className="w-full flex-grow overflow-y-scroll">
          <div className="p-8">
            <Switch>
              <Route path="/general">
                <GeneralSettings />
              </Route>
              <Route path="/spaces"></Route>
              <Route path="/explorer"></Route>
            </Switch>
          </div>

          {/*<div className="flex flex-row mt-4 space-x-2">*/}
          {/*  <Toggle initialState={false} />*/}
          {/*</div>*/}

          {/*<Dropdown*/}
          {/*  buttonProps={{}}*/}
          {/*  buttonText="My Library"*/}
          {/*  items={[*/}
          {/*    [*/}
          {/*      { name: 'Edit', icon: PencilAltIcon },*/}
          {/*      { name: 'Copy', icon: DuplicateIcon }*/}
          {/*    ],*/}
          {/*    [{ name: 'Delete', icon: TrashIcon }]*/}
          {/*  ]}*/}
          {/*/>*/}
        </div>
      </div>
    </Router>
  );
};
