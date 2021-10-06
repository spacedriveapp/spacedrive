import React, { useRef, useState } from 'react';
import { CookingPot, Copy, Gear, Pencil, TrashSimple } from 'phosphor-react';

import { invoke } from '@tauri-apps/api';
import { Button } from './components/primative/Button';
import { Input, Toggle } from './components/primative';
import { InputContainer } from './components/primative/InputContainer';
import { TrafficLights } from './components/os/TrafficLights';
import { Checkbox } from './components/primative/Checkbox';
import { useInputState } from './hooks/useInputState';
import { Dropdown } from './components/primative/Dropdown';
import { DuplicateIcon, PencilAltIcon, TrashIcon } from '@heroicons/react/solid';
import { FileRow } from './components/file/FileRow';
import { Sidebar } from './components/file/Sidebar';

export default function App() {
  const fileUploader = useRef<HTMLInputElement | null>(null);
  const inputState = useInputState('/Users/jamie/Downloads');

  return (
    <div className="flex flex-col h-screen rounded-xl border border-gray-200 dark:border-gray-600 bg-white text-gray-900 dark:text-white dark:bg-gray-800 overflow-hidden ">
      <div
        data-tauri-drag-region
        className="flex flex-grow flex-shrink-0 max-w items-center bg-gray-50 dark:bg-gray-900 h-8 border-gray-100 dark:border-gray-900 shadow-sm justify-between dark:border-t "
      >
        <TrafficLights className="p-1.5" />
        <Button noBorder noPadding className="mr-2">
          <Gear weight="fill" />
        </Button>
      </div>
      <div className="h-[1px] flex-shrink-0 max-w bg-gray-100 dark:bg-gray-700" />
      <div className="flex flex-row min-h-full">
        <Sidebar />
        <div className="px-6 py-4">
          <div className="flex space-x-2">
            <InputContainer
              title="Quick scan directory"
              description="The directory for which this application will perform a detailed scan of the contents and sub directories"
            >
              <Input {...inputState} placeholder="/users/jamie/Desktop" />
            </InputContainer>
            <InputContainer
              title="Quick scan directory"
              description="The directory for which this application will perform a detailed scan of the contents and sub directories"
            >
              <Input {...inputState} placeholder="/users/jamie/Desktop" />
            </InputContainer>
          </div>
          <div className="space-x-2 flex flex-row mt-6">
            <Button
              variant="primary"
              onClick={() => {
                invoke('scan_dir', {
                  path: inputState.value
                });
              }}
            >
              Scan Now
            </Button>
            <Button>Cancel</Button>
          </div>
          <div className="flex space-x-2 mt-2">
            <Button size="sm" variant="primary">
              Cancel
            </Button>
            <Button size="sm">Cancel</Button>
          </div>
          <div className="space-x-2 flex flex-row mt-4">
            <Toggle initialState={false} />
          </div>
          <div className="space-x-2 flex flex-row mt-4 mb-5 ml-1">
            <Checkbox />
            <Checkbox />
            <Checkbox />
          </div>
          <Dropdown
            buttonText="My Library"
            items={[
              [
                { name: 'Edit', icon: PencilAltIcon },
                { name: 'Copy', icon: DuplicateIcon }
              ],
              [{ name: 'Delete', icon: TrashIcon }]
            ]}
          />
        </div>
        <div className="px-6 mt-4">
          <FileRow />
        </div>
      </div>
    </div>
  );
}
