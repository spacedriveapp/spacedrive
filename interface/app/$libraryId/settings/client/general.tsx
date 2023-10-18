import { Laptop } from '@sd/assets/icons';
import { useWatch } from 'react-hook-form';
import {
	getDebugState,
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useFeatureFlag,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, InputField, Switch, SwitchField, tw, z } from '@sd/ui';
import { useDebouncedFormWatch } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';
import { SpacedriveAccount } from './SpacedriveAccount';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

// https://doc.rust-lang.org/std/u16/index.html
const u16 = z.number().min(0).max(65_535);

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');
	const p2pSettingsEnabled = useFeatureFlag('p2pSettings');
	const connectedPeers = useConnectedPeers();

	const form = useZodForm({
		schema: z.object({
			name: z.string().min(1).optional(),
			p2p_enabled: z.boolean().optional(),
			p2p_port: u16.optional().nullish()
		}),
		defaultValues: {
			name: node.data?.name,
			p2p_enabled: node.data?.p2p_enabled,
			p2p_port: node.data?.p2p_port
		}
	});

	useDebouncedFormWatch(form, async (value) => {
		await editNode.mutateAsync({
			name: value.name || null,
			p2p_enabled: value.p2p_enabled === undefined ? null : value.p2p_enabled,
			// @ts-expect-error: Specta can't properly express this type. - https://github.com/oscartbeaumont/specta/issues/157
			p2p_port: value.p2p_port
		});

		node.refetch();
	});

	console.log(node.data); // TODO: remove

	return (
		<>
			<Heading
				title="General Settings"
				description="General settings related to this client."
			/>
			<SpacedriveAccount />
			<Card className="px-5">
				<div className="my-2 flex w-full flex-col">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">Local Node</span>
						<div className="flex flex-row space-x-1">
							<NodePill>{connectedPeers.size} Peers</NodePill>
							{node.data?.p2p_enabled === true ? (
								<NodePill className="!bg-accent text-white">Running</NodePill>
							) : (
								<NodePill className="text-white">Disabled</NodePill>
							)}
						</div>
					</div>

					<hr className="mb-4 mt-2 flex  w-full border-app-line" />
					<div className="flex w-full items-center gap-5">
						<img src={Laptop} className="mt-2 h-14 w-14" />

						<div className="flex flex-col">
							<NodeSettingLabel>Node Name</NodeSettingLabel>
							<Input
								{...form.register('name', { required: true })}
								defaultValue={node.data?.name}
							/>
						</div>
					</div>

					<div className="mt-6 gap-2">
						{/* <div
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
						</div> */}

						<div>
							<NodeSettingLabel>Data Folder</NodeSettingLabel>
							<div className="mt-2 flex w-full flex-row gap-2">
								<Input className="grow" value={node.data?.data_path} disabled />
								<Button
									size="sm"
									variant="outline"
									onClick={() => {
										if (node.data && !!platform?.openLink) {
											platform.confirm(
												'Modifying or backing up data within this folder may cause irreparable damage! Proceed at your own risk!',
												(result) => {
													if (result) {
														platform.openLink(node.data.data_path);
													}
												}
											);
										}
									}}
								>
									Open
								</Button>
								{/* <Button size="sm" variant="outline">
									Change
								</Button> */}
							</div>
						</div>
						{/* <div className='mb-1'>
							<Label className="text-sm font-medium text-ink-faint">
								<Database className="mr-1 mt-[-2px] inline h-4 w-4" /> Logs Folder
							</Label>
							<Input value={node.data?.data_path + '/logs'} />
						</div> */}
					</div>
					{/* <div className="pointer-events-none mt-5 flex items-center space-x-3 opacity-50">
						<Switch size="sm" />
						<span className="text-sm font-medium text-ink-dull">
							Run Spacedrive in the background when app closed
						</span>
					</div> */}
				</div>
			</Card>

			<Setting
				mini
				title="Debug mode"
				description="Enable extra debugging features within the app."
			>
				<Switch
					size="md"
					checked={debugState.enabled}
					onClick={() => (getDebugState().enabled = !debugState.enabled)}
				/>
			</Setting>
			{p2pSettingsEnabled && (
				<div className="flex flex-col gap-4">
					<h1 className="mb-3 text-lg font-bold text-ink">Networking</h1>

					<Setting
						mini
						title="Enable Networking"
						description={
							<>
								<p className="text-sm text-gray-400">
									Allow your node to communicate with other Spacedrive nodes
									around you
								</p>
								<p className="mb-2 text-sm text-gray-400">
									<span className="font-bold">Required</span> for library sync or
									Spacedrop!
								</p>
							</>
						}
					>
						{/* TODO: Switch doesn't handle optional fields correctly */}
						<Switch
							size="md"
							checked={form.watch('p2p_enabled') || false}
							onClick={() =>
								form.setValue('p2p_enabled', !form.getValues('p2p_enabled'))
							}
						/>
					</Setting>
					{/* TODO: Input field doesn't handle optional or nullable correctly */}
					{/* TODO: How should we express `Option<u16>`. Maybe a dropdown with a "Default" and input field in it? */}
					{/* <Setting
						mini
						title="Networking Port"
						description="The port for Spacedrive's Peer-to-peer networking to communicate on\nYou should leave this disabled unless you have a restictive firewall.\nDo not expose to the internet! "
					>
						<InputField {...form.register('p2p_port')} />
					</Setting> */}
				</div>
			)}
		</>
	);
};
