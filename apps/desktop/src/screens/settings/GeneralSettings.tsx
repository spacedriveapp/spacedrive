import { InputContainer } from '../../components/primitive/InputContainer';
import { Button, Input } from '../../components/primitive';
import { invoke } from '@tauri-apps/api';
import React, { useRef } from 'react';
import { useExplorerStore } from '../../store/explorer';
import { useAppState } from '../../store/global';
import Listbox from '../../components/primitive/Listbox';
import { useLocations } from '../../store/locations';

export default function GeneralSettings() {
  const locations = useLocations();

  const fileUploader = useRef<HTMLInputElement | null>(null);
  const config = useAppState();

  const [tempWatchDir, setTempWatchDir] = useExplorerStore((state) => [
    state.tempWatchDir,
    state.setTempWatchDir
  ]);

  return (
    <div className="space-y-4">
      <div className="mb-6">
        <h1 className="text-2xl font-bold">General Settings</h1>
        <p className="text-sm mt-1 text-gray-400">Basic settings related to this client</p>
        <hr className="border-gray-550 mt-4" />
      </div>
      <InputContainer
        title="Quick scan directory"
        description="The directory for which this application will perform a detailed scan of the contents and sub directories"
      >
        <div className="flex flex-row">
          <Input
            value={tempWatchDir}
            size="sm"
            className="flex-grow"
            onChange={(e) => setTempWatchDir(e.target.value)}
            placeholder="/users/jamie/Desktop"
          />
          <Button
            className="ml-2"
            size="sm"
            variant="primary"
            onClick={async () => {
              await invoke('scan_dir', {
                path: tempWatchDir
              });
            }}
          >
            Scan Now
          </Button>
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input className="flex-grow" value={config.data_dir} placeholder="/users/jamie/Desktop" />
        </div>
      </InputContainer>
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Button variant="primary">Enable Vault</Button>
          {/*<Input className="flex-grow" value="jeff" placeholder="/users/jamie/Desktop" />*/}
        </div>
      </InputContainer>
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row space-x-2">
          <div className="flex flex-grow">
            <Listbox
              options={locations.map((location) => ({
                key: location.name,
                option: location.name,
                description: location.path
              }))}
            />
          </div>
          <Button className="mb-3" variant="primary">
            Add Location
          </Button>
        </div>
      </InputContainer>
    </div>
  );
}
