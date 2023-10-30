import clsx from 'clsx';
import { useEffect } from 'react';
import { Controller } from 'react-hook-form';
import {
	getDebugState,
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useFeatureFlag,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Select, SelectOption, Switch, tw, z } from '@sd/ui';
import { Icon } from '~/components';
import { useDebouncedFormWatch } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

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
			p2p_port: u16,
			customOrDefault: z.enum(['Custom', 'Default'])
		}),
		reValidateMode: 'onChange',
		defaultValues: {
			name: node.data?.name,
			p2p_enabled: node.data?.p2p_enabled,
			p2p_port: node.data?.p2p_port || 0,
			customOrDefault: node.data?.p2p_port ? 'Custom' : 'Default'
		}
	});

	const watchCustomOrDefault = form.watch('customOrDefault');
	const watchP2pEnabled = form.watch('p2p_enabled');

	useDebouncedFormWatch(form, async (value) => {
		await editNode.mutateAsync({
			name: value.name || null,
			p2p_enabled: value.p2p_enabled === undefined ? null : value.p2p_enabled,
			p2p_port: value.customOrDefault === 'Default' ? 0 : Number(value.p2p_port)
		});

		node.refetch();
	});

	useEffect(() => {
		form.watch((data) => {
			if (Number(data.p2p_port) > 65535) {
				form.setValue('p2p_port', 65535);
			}
		});
	}, [form]);

	return (
		<>
			<Heading
				title="General Settings"
				description="General settings related to this client."
			/>
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

					<hr className="mb-4 mt-2 flex w-full border-app-line" />
					<div className="flex w-full items-center gap-5">
						<Icon name="Laptop" className="mt-2 h-14 w-14" />
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
							<b className="inline mr-2 truncate">
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
					{/* <div className="flex items-center mt-5 space-x-3 opacity-50 pointer-events-none">
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
							checked={watchP2pEnabled || false}
							onClick={() =>
								form.setValue('p2p_enabled', !form.getValues('p2p_enabled'))
							}
						/>
					</Setting>
					<Setting
						mini
						title="Networking Port"
						description="The port for Spacedrive's Peer-to-peer networking to communicate on. You should leave this disabled unless you have a restictive firewall. Do not expose to the internet!"
					>
						<div className="flex gap-2">
							<Controller
								control={form.control}
								name="customOrDefault"
								render={({ field }) => (
									<Select
										disabled={!watchP2pEnabled}
										className={clsx(!watchP2pEnabled && 'opacity-50')}
										{...field}
										onChange={(e) => {
											field.onChange(e);
											form.setValue('p2p_port', 0);
										}}
									>
										<SelectOption value="Default">Default</SelectOption>
										<SelectOption value="Custom">Custom</SelectOption>
									</Select>
								)}
							/>
							<Input
								className={clsx(
									'w-[66px]',
									watchCustomOrDefault === 'Default' || !watchP2pEnabled
										? 'opacity-50'
										: 'opacity-100'
								)}
								disabled={watchCustomOrDefault === 'Default' || !watchP2pEnabled}
								{...form.register('p2p_port')}
								onChange={(e) => {
									form.setValue(
										'p2p_port',
										Number(e.target.value.replace(/[^0-9]/g, ''))
									);
								}}
							/>
						</div>
					</Setting>
				</div>
			)}
		</>
	);
};
