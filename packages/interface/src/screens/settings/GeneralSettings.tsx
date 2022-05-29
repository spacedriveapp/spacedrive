import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { Input } from '@sd/ui';
import React, { useState } from 'react';

import { InputContainer } from '../../components/primitive/InputContainer';
import Listbox from '../../components/primitive/Listbox';
import Slider from '../../components/primitive/Slider';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

export default function GeneralSettings() {
	const { data: volumes } = useBridgeQuery('SysGetVolumes');

	return (
		<SettingsContainer>
			<SettingsHeader
				title="General Settings"
				description="Basic settings related to this client."
			/>
			<p className="px-5 py-3 mb-3 text-sm text-gray-400 rounded-md bg-gray-50 dark:text-gray-400 dark:bg-gray-600">
				<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
				functional.
			</p>

			{/* <InputContainer
        title="Test scan directory"
        description="This will create a job to scan the directory you specify to the database."
      >
        <div className="flex flex-row">
          <Input
            value={tempWatchDir}
            className="flex-grow"
            onChange={(e) => setTempWatchDir(e.target.value)}
            placeholder="/users/jamie/Desktop"
          />
          <Button
            className="ml-2"
            variant="primary"
            onClick={() =>
              createLocation({
                path: tempWatchDir
              })
            }
          >
            Scan Now
          </Button>
        </div>
      </InputContainer> */}

			<InputContainer title="Volumes" description="A list of volumes running on this device.">
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
				</div>
			</InputContainer>

			{/* <div className="">{JSON.stringify({ config })}</div> */}
		</SettingsContainer>
	);
}
