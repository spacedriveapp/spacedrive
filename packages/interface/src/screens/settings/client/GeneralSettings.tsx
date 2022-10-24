import { useBridgeQuery, usePlatform } from '@sd/client';
import { Card, Input, Switch, tw } from '@sd/ui';
import { Database } from 'phosphor-react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

export default function GeneralSettings() {
	const { data: node } = useBridgeQuery(['getNode']);

	const platform = usePlatform();

	return (
		<SettingsContainer>
			<SettingsHeader
				title="General Settings"
				description="General settings related to this client."
			/>
			<Card className="px-5">
				<div className="flex flex-col w-full my-2">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">Connected Node</span>
						<div className="flex flex-row space-x-1">
							<NodePill>0 Peers</NodePill>
							<NodePill className="text-white !bg-accent">Running</NodePill>
						</div>
					</div>

					<hr className="mt-2 mb-4 border-app-line" />
					<div className="grid grid-cols-3 gap-2">
						<div className="flex flex-col">
							<NodeSettingLabel>Node Name</NodeSettingLabel>
							<Input value={node?.name} />
						</div>
						<div className="flex flex-col ">
							<NodeSettingLabel>Node Port</NodeSettingLabel>
							<Input contentEditable={false} value={node?.p2p_port || 5795} />
						</div>
					</div>
					<div className="flex items-center mt-5 space-x-3">
						<Switch size="sm" checked />
						<span className="text-sm text-ink-dull">Run daemon when app closed</span>
					</div>
					<div className="mt-3">
						<div
							onClick={() => {
								if (node && platform?.openLink) {
									platform.openLink(node.data_path);
								}
							}}
							className="text-xs font-medium leading-relaxed text-ink-faint"
						>
							<b className="inline mr-2 truncate ">
								<Database className="inline w-4 h-4 mr-1 -mt-[2px]" /> Data Folder
							</b>
							<span className="select-text">{node?.data_path}</span>
						</div>
					</div>
				</div>
			</Card>
		</SettingsContainer>
	);
}
