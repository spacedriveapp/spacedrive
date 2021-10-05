import React, { useRef, useState } from 'react';
import { CookingPot } from 'phosphor-react';
import { invoke } from '@tauri-apps/api';
import { Button } from './components/primative/Button';
import { Input, Toggle } from './components/primative';
import { InputContainer } from './components/primative/InputContainer';
import { TrafficLights } from './components/os/trafficLights';

export default function App() {
  const fileUploader = useRef<HTMLInputElement | null>(null);
  const [fileInputVal, setFileInputVal] = useState('/Users/jamie/Downloads/lol.mkv');
  const [fileScanInputVal, setFileScanInputVal] = useState('/Users/jamie/Downloads');

  return (
    <div className="flex flex-col h-screen rounded-xl bg-white dark:bg-gray-900 overflow-hidden ">
      <div
        data-tauri-drag-region
        className="max-w p-2 bg-gray-50 dark:bg-gray-800 h-10 border-b border-gray-100 dark:border-gray-900 shadow-sm"
      >
        <TrafficLights className="p-1.5" />
      </div>
      <div className="p-4 ">
        <div className="flex space-x-2">
          <InputContainer title="Quick scan directory">
            <Input placeholder="/users/jamie/Desktop" />
          </InputContainer>
        </div>
        <div className="space-x-2 flex flex-row mt-2">
          <Button
            variant="primary"
            onClick={() => {
              invoke('scan_dir', {
                path: fileScanInputVal
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
      </div>
    </div>
  );
}
