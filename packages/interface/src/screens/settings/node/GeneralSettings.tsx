import { useBridgeQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import React from 'react';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import Listbox from '../../../components/primitive/Listbox';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function GeneralSettings() {
	const { data: volumes } = useBridgeQuery('SysGetVolumes');

	return (
		<SettingsContainer>
			<SettingsHeader
				title="General Settings"
				description="General settings related to this node."
			/>
			{/* <InputContainer title="Volumes" description="A list of volumes running on this device.">
				<div className="flex flex-row space-x-2">
					<div className="flex flex-grow">
						<Listbox
							options={
								volumes?.map((volume) => {
									const name = volume.name && volume.name.length ? volume.name : volume.mount_point;
									return {
										key: name,
										option: name,
										description: volume.mount_point
									};
								}) ?? []
							}
						/>
					</div>
				</div>
			</InputContainer> */}

			<InputContainer
				mini
				title="Enable Node Discovery"
				description="Allow or block this node from calling an external server to assist in forming a peer-to-peer connection. "
			>
				<Toggle value />
			</InputContainer>

			<InputContainer
				title="Discovery Server"
				description="Configuration server to aid with establishing peer-to-peer to connections between nodes over the internet. Disabling will result in nodes only being accessible over LAN and direct IP connections."
			>
				<div className="flex flex-col mt-1">
					<Input className="flex-grow" disabled defaultValue="https://p2p.spacedrive.com" />
					<div className="flex justify-end mt-1">
						<a className="p-1 text-sm font-bold text-primary-500 hover:text-primary-400">Change</a>
					</div>
				</div>
			</InputContainer>

			{/* <div className="">{JSON.stringify({ config })}</div> */}
		</SettingsContainer>
	);
}
