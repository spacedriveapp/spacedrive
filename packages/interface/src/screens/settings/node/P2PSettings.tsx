import { Input, Switch } from '@sd/ui';
import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

export default function P2PSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader
				title="P2P Settings"
				description="Manage how this node communicates with other nodes."
			/>

			<InputContainer
				mini
				title="Enable Node Discovery"
				description="Allow or block this node from calling an external server to assist in forming a peer-to-peer connection. "
			>
				<Switch checked />
			</InputContainer>

			<InputContainer
				title="Discovery Server"
				description="Configuration server to aid with establishing peer-to-peer to connections between nodes over the internet. Disabling will result in nodes only being accessible over LAN and direct IP connections."
			>
				<div className="mt-1 flex flex-col">
					<Input className="flex-grow" disabled defaultValue="https://p2p.spacedrive.com" />
					<div className="mt-1 flex justify-end">
						<a className="text-accent hover:text-accent p-1 text-sm font-bold">Change</a>
					</div>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
