import { Database } from 'phosphor-react';
import { getDebugState, useBridgeQuery, useDebugState } from '@sd/client';
import { Card, Input, Switch, tw } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { Heading } from '../Layout';
import Setting from '../Setting';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();

	return (
		<>
			<Heading
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

					<hr className="mb-4 mt-2 border-app-line" />
					<div className="grid grid-cols-3 gap-2">
						<div className="flex flex-col">
							<NodeSettingLabel>Node Name</NodeSettingLabel>
							<Input
								value={node.data?.name}
								onChange={() => {
									/* TODO */
								}}
								disabled
							/>
						</div>
						<div className="flex flex-col">
							<NodeSettingLabel>Node Port</NodeSettingLabel>
							<Input
								contentEditable={false}
								value={node.data?.p2p_port || 5795}
								onChange={() => {
									/* TODO */
								}}
								disabled
							/>
						</div>
					</div>
					<div className="mt-5 flex items-center space-x-3">
						<Switch size="sm" checked />
						<span className="text-sm font-medium text-ink-dull">
							Run daemon when app closed
						</span>
					</div>
					<div className="mt-3">
						<div
							onClick={() => {
								if (node.data && platform?.openLink) {
									platform.openLink(node.data.data_path);
								}
							}}
							className="text-sm font-medium text-ink-faint"
						>
							<b className="mr-2 inline truncate">
								<Database className="mr-1 mt-[-2px] inline h-4 w-4" /> Data Folder
							</b>
							<span className="select-text">{node.data?.data_path}</span>
						</div>
					</div>
				</div>
			</Card>
			{isDev && (
				<Setting
					mini
					title="Debug mode"
					description="Enable extra debugging features within the app."
				>
					<Switch
						checked={debugState.enabled}
						onClick={() => (getDebugState().enabled = !debugState.enabled)}
					/>
				</Setting>
			)}
		</>
	);
};
