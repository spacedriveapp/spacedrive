import { DuplicateIcon, PencilAltIcon, TrashIcon } from '@heroicons/react/solid';
import { invoke } from '@tauri-apps/api';
import React, { useRef } from 'react';
// import { dummyFileData, FileList } from '../components/file/FileList';
import { Input, Toggle } from '../components/primative';
import { Button } from '../components/primative/Button';
import { Checkbox } from '../components/primative/Checkbox';
import { Dropdown } from '../components/primative/Dropdown';
import { InputContainer } from '../components/primative/InputContainer';
import { Shortcut } from '../components/primative/Shortcut';
import { useInputState } from '../hooks/useInputState';

export const SettingsScreen: React.FC<{}> = () => {
  const fileUploader = useRef<HTMLInputElement | null>(null);
  const inputState = useInputState('/Users/jamie/Downloads');

  return (
    <div>
      <div className="p-3">
        {/* <FileList files={dummyFileData} /> */}
        <div className="flex space-x-2 mt-4">
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
          buttonProps={{}}
          buttonText="My Library"
          items={[
            [
              { name: 'Edit', icon: PencilAltIcon },
              { name: 'Copy', icon: DuplicateIcon }
            ],
            [{ name: 'Delete', icon: TrashIcon }]
          ]}
        />
        <div className="mt-3 space-x-1">
          <Shortcut chars="⌘" />
          <Shortcut chars="S" />
        </div>
      </div>
    </div>
  );
};
