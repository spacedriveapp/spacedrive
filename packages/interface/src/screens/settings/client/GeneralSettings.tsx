import { Database } from 'phosphor-react';
import { getDebugState, useBridgeQuery, useDebugState } from '@sd/client';
import { Card, Input, Switch, tw } from '@sd/ui';
import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';
import { usePlatform } from '~/util/Platform';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

export default function GeneralSettings() {
	const { data: node } = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();

	return (
		<SettingsContainer>
			<SettingsHeader
				title="General Settings"
				description="General settings related to this client."
			/>
			<Card className="px-5">
				<div className="my-2 flex w-full flex-col">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">Connected Node</span>
						<div className="flex flex-row space-x-1">
							<NodePill>0 Peers</NodePill>
							<NodePill className="!bg-accent text-white">Running</NodePill>
						</div>
					</div>

					<hr className="border-app-line mt-2 mb-4" />
					<div className="grid grid-cols-3 gap-2">
						<div className="flex flex-col">
							<NodeSettingLabel>Node Name</NodeSettingLabel>
							<Input value={node?.name} />
						</div>
						<div className="flex flex-col">
							<NodeSettingLabel>Node Port</NodeSettingLabel>
							<Input contentEditable={false} value={node?.p2p_port || 5795} />
						</div>
					</div>
					<div className="mt-5 flex items-center space-x-3">
						<Switch size="sm" checked />
						<span className="text-ink-dull text-sm font-medium">Run daemon when app closed</span>
					</div>
					<div className="mt-3">
						<div
							onClick={() => {
								if (node && platform?.openLink) {
									platform.openLink(node.data_path);
								}
							}}
							className="text-ink-faint text-sm font-medium"
						>
							<b className="mr-2 inline truncate">
								<Database className="mr-1 mt-[-2px] inline h-4 w-4" /> Data Folder
							</b>
							<span className="select-text">{node?.data_path}</span>
						</div>
					</div>
				</div>
			</Card>
			<InputContainer
				mini
				title="Debug mode"
				description="Enable extra debugging features within the app."
			>
				<Switch
					checked={debugState.enabled}
					onClick={() => (getDebugState().enabled = !debugState.enabled)}
				/>
			</InputContainer>
		</SettingsContainer>
	);
}
