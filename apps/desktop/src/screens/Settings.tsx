import { DuplicateIcon, PencilAltIcon, TrashIcon } from '@heroicons/react/solid';
import { invoke } from '@tauri-apps/api';
import React, { useRef } from 'react';
// import { dummyIFile, FileList } from '../components/file/FileList';
import { Input, Toggle } from '../components/primitive';
import { Button } from '../components/primitive/Button';
import { Checkbox } from '../components/primitive/Checkbox';
import { Dropdown } from '../components/primitive/Dropdown';
import { InputContainer } from '../components/primitive/InputContainer';
import { Shortcut } from '../components/primitive/Shortcut';
import { useInputState } from '../hooks/useInputState';
import { useExplorerStore } from '../store/explorer';
//@ts-ignore
// import { Spline } from 'react-spline';
// import WINDOWS_SCENE from '../assets/spline/scene.json';

export const SettingsScreen: React.FC<{}> = () => {
  const fileUploader = useRef<HTMLInputElement | null>(null);

  const [tempWatchDir, setTempWatchDir] = useExplorerStore((state) => [
    state.tempWatchDir,
    state.setTempWatchDir
  ]);

  return (
    <div>
      <div className="px-5">
        {/* <FileList files={dummyIFile} /> */}
        {/* <Spline scene={WINDOWS_SCENE} /> */}
        {/* <iframe
          src="https://my.spline.design/windowscopy-8e92a2e9b7cb4d9237100441e8c4f688/"
          width="100%"
          height="100%"
        ></iframe> */}
        <div className="flex space-x-2 mt-4">
          <InputContainer
            title="Quick scan directory"
            description="The directory for which this application will perform a detailed scan of the contents and sub directories"
          >
            <Input
              value={tempWatchDir}
              onChange={(e) => setTempWatchDir(e.target.value)}
              placeholder="/users/jamie/Desktop"
            />
          </InputContainer>
        </div>
        <div className="space-x-2 flex flex-row mt-2">
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              invoke('scan_dir', {
                path: tempWatchDir
              });
            }}
          >
            Scan Now
          </Button>
          <Button
            size="sm"
            onClick={() => {
              invoke('test_scan');
            }}
          >
            Test Scan
          </Button>
          <Button size="sm">Test</Button>
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
