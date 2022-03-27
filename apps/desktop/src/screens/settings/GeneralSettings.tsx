import { InputContainer } from '../../components/primitive/InputContainer';
import { Button, Input } from '../../components/primitive';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useState } from 'react';

import Listbox from '../../components/primitive/Listbox';

import ReactJson from 'react-json-view';
import Slider from '../../components/primitive/Slider';
import { useBridgeCommand, useBridgeQuery } from '@sd/state';

export default function GeneralSettings() {
  const { data: volumes } = useBridgeQuery('SysGetVolumes');
  const [tempWatchDir, setTempWatchDir] = useState('/users/jamie/Projects');

  const [fakeSliderVal, setFakeSliderVal] = useState([30, 0]);

  const { mutate } = useBridgeCommand('LocCreate');

  // const fileUploader = useRef<HTMLInputElement | null>(null);
  const { data: client } = useBridgeQuery('ClientGetState');
  const { data: jobs } = useBridgeQuery('JobGetRunning');
  const { data: jobsHistory } = useBridgeQuery('JobGetHistory');

  return (
    <div className="flex flex-col max-w-2xl space-y-4">
      <div className="mt-3 mb-6">
        <h1 className="text-2xl font-bold">General Settings</h1>
        <p className="mt-1 text-sm text-gray-400">Basic settings related to this client</p>
        {/* <hr className="mt-4 border-gray-550" /> */}
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
            variant="primary"
            onClick={async () => {
              // await invoke('scan_dir', {
              //   path: client?.data_path
              // });
              mutate({
                path: tempWatchDir
              });
            }}
          >
            Scan Now
          </Button>
        </div>
      </InputContainer>
      <ReactJson
        // collapsed
        enableClipboard={false}
        displayDataTypes={false}
        theme="ocean"
        src={{ ...jobs }}
        style={{
          padding: 20,
          borderRadius: 5,
          backgroundColor: '#101016',
          border: 1,
          borderColor: '#1E1E27',
          borderStyle: 'solid'
        }}
      />
      <ReactJson
        // collapsed
        enableClipboard={false}
        displayDataTypes={false}
        theme="ocean"
        src={{ ...jobsHistory }}
        style={{
          padding: 20,
          borderRadius: 5,
          backgroundColor: '#101016',
          border: 1,
          borderColor: '#1E1E27',
          borderStyle: 'solid'
        }}
      />
      <InputContainer
        title="Locations"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row space-x-2">
          <div className="flex flex-grow">
            <Listbox
              options={
                volumes?.map((volume) => ({
                  key: volume.name,
                  option: volume.name,
                  description: volume.mount_point
                })) ?? []
              }
            />
          </div>
          <Button className="mb-3" variant="primary">
            Add Location
          </Button>
        </div>
      </InputContainer>
      <InputContainer
        title="Volumes"
        description="A list of mounted volumes on this machine, for no reason."
      >
        <Slider
          step={5}
          value={fakeSliderVal}
          onValueChange={setFakeSliderVal}
          defaultValue={[25, 75]}
        />
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="flex-grow"
            value={'uuuuuu'}
            placeholder="/users/jamie/Library/Application Support/spacedrive/cache"
          />
        </div>
      </InputContainer>
      <InputContainer title="Vault" description="Enable vault storage with VeraCrypt.">
        <div className="flex flex-row">
          <Button variant="primary">Enable Vault</Button>
          {/*<Input className="flex-grow" value="jeff" placeholder="/users/jamie/Desktop" />*/}
        </div>
      </InputContainer>

      {/* <div className="">{JSON.stringify({ config })}</div> */}
    </div>
  );
}
