import clsx from 'clsx';
import { PropsWithChildren, useState } from 'react';
import { FormProvider } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { Link } from 'react-router-dom';
import { match } from 'ts-pattern';
import { z } from 'zod';
import {
	ListenerState,
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useFeatureFlag,
	usePeers,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Select, SelectOption, Switch, toast, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useDebouncedFormWatch, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../../Layout';
import Setting from '../../Setting';
import { NodePill } from '../general';

const u16 = () => z.number().min(0).max(65535);

const socketAddrRegex =
	/^(([0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3})|([a-zA-Z.]+))(:[0-9]{1,5})?$/;

function RenderListenerPill(props: PropsWithChildren<{ listener?: ListenerState }>) {
	if (props.listener?.type === 'Error') {
		return (
			<Tooltip label={`Error: ${props.listener.error}`}>
				<NodePill className="bg-red-700">{props.children}</NodePill>
			</Tooltip>
		);
	} else if (props.listener?.type === 'Listening') {
		return <NodePill className="bg-green-700">{props.children}</NodePill>;
	}
	return <NodePill>{props.children}</NodePill>;
}

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const listeners = useBridgeQuery(['p2p.listeners'], {
		refetchInterval: 1000
	});
	const editNode = useBridgeMutation('nodes.edit');
	const connectedPeers = useConnectedPeers();
	const [newSocket, setNewSocket] = useState<string>('');

	const { t } = useLocale();

	const form = useZodForm({
		schema: z
			.object({
				port: z.discriminatedUnion('type', [
					z.object({ type: z.literal('random') }),
					z.object({ type: z.literal('discrete'), value: u16() })
				]),
				disabled: z.boolean().optional(),
				ipv6_disabled: z.boolean().optional(),
				relay_disabled: z.boolean().optional(),
				discovery: z
					.union([
						z.literal('Everyone'),
						z.literal('ContactsOnly'),
						z.literal('Disabled')
					])
					.optional(),
				enable_remote_access: z.boolean().optional(),
				p2p_manual_peers: z.array(z.string()).optional()
			})
			.strict(),
		reValidateMode: 'onChange',
		defaultValues: {
			port: node.data?.p2p.port || { type: 'random' },
			disabled: node.data?.p2p.disabled || false,
			ipv6_disabled: node.data?.p2p.disable_ipv6 || false,
			relay_disabled: node.data?.p2p.disable_relay || false,
			discovery: node.data?.p2p.discovery || 'Everyone',
			enable_remote_access: node.data?.p2p.disable_relay || false,
			p2p_manual_peers: node.data?.p2p.manual_peers || []
		}
	});

	useDebouncedFormWatch(form, async (value) => {
		if (await form.trigger()) {
			await editNode.mutateAsync({
				name: null,
				p2p_port: (value.port as any) ?? null,
				p2p_disabled: value.disabled ?? null,
				p2p_ipv6_disabled: value.ipv6_disabled ?? null,
				p2p_relay_disabled: value.relay_disabled ?? null,
				p2p_discovery: value.discovery ?? null,
				p2p_remote_access: value.enable_remote_access ?? null,
				p2p_manual_peers: value.p2p_manual_peers?.flatMap((v) => (v ? [v] : [])) ?? null
				// image_labeler_version: null
			});
		}

		node.refetch();
	});

	const port = form.watch('port');
	form.watch((data) => {
		if (data.port?.type == 'discrete' && Number(data.port.value) > 65535) {
			form.setValue('port', { type: 'discrete', value: 65535 });
		}
	});

	const isP2PWipFeatureEnabled = useFeatureFlag('wipP2P');

	const isNewSocketInvalid = socketAddrRegex.test(newSocket) === false;

	return (
		<FormProvider {...form}>
			<Heading
				title={t('network_settings')}
				description={t('network_settings_description')}
				rightArea={
					<Link to="./debug" className="text-xs">
						{t('advanced')}
					</Link>
				}
			/>

			<Card className="flex flex-col px-5 pb-4">
				<div className="my-2 flex w-full flex-col">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">{node.data?.name}</span>
						<div className="flex flex-row space-x-1">
							<RenderListenerPill listener={listeners.data?.ipv4}>
								IPv4
							</RenderListenerPill>
							<RenderListenerPill listener={listeners.data?.ipv6}>
								IPv6
							</RenderListenerPill>
							{match(
								node.data?.p2p.disabled
									? 'Disabled'
									: node.data?.p2p.discovery || 'Disabled'
							)
								.with('Disabled', () => <NodePill>LAN</NodePill>)
								.with('ContactsOnly', () => (
									<Tooltip label="Only discoverable by contacts">
										<NodePill className="bg-orange-700">LAN</NodePill>
									</Tooltip>
								))
								.with('Everyone', () => (
									<NodePill className="bg-green-700">LAN</NodePill>
								))
								.exhaustive()}
							<RenderListenerPill listener={listeners.data?.relay}>
								Relay
							</RenderListenerPill>
						</div>
					</div>
				</div>

				<div>
					<p>
						{t('remote_identity')}: {node.data?.identity}
					</p>
				</div>
			</Card>

			<Setting
				mini
				title={t('enable_networking')}
				description={
					<>
						<p className="text-sm text-gray-400">
							{t('enable_networking_description')}
						</p>
						<p className="mb-2 text-sm text-gray-400">
							{t('enable_networking_description_required')}
						</p>
					</>
				}
			>
				<Switch
					size="md"
					checked={!form.watch('disabled')}
					onCheckedChange={(checked) => form.setValue('disabled', !checked)}
				/>
			</Setting>

			{!form.watch('disabled') ? (
				<>
					<Setting
						mini
						title={t('networking_port')}
						description={t('networking_port_description')}
					>
						<div className="flex h-[30px] gap-2">
							{node.data?.is_in_docker === true ? (
								<Tooltip label="This port is hardcoded in the container but configurable via the Docker `-p` option">
									<Input value="7373" disabled />
								</Tooltip>
							) : (
								<>
									<Select
										value={port.type}
										containerClassName="h-[30px]"
										className="h-full"
										onChange={(type) => {
											form.setValue('port', {
												type: type as any
											});
										}}
									>
										<SelectOption value="random">{t('random')}</SelectOption>
										<SelectOption value="discrete">{t('custom')}</SelectOption>
									</Select>
									<Input
										value={port.type === 'discrete' ? port.value : 0}
										className={clsx(
											'w-[66px]',
											port.type === 'random' ? 'opacity-50' : 'opacity-100'
										)}
										disabled={port.type === 'random'}
										onChange={(e) => {
											form.setValue('port', {
												type: 'discrete',
												value: Number(e.target.value.replace(/[^0-9]/g, ''))
											});
										}}
									/>
								</>
							)}
						</div>
					</Setting>

					<Setting
						mini
						title={t('ipv6')}
						description={
							<p className="text-sm text-gray-400">{t('ipv6_description')}</p>
						}
					>
						<Switch
							size="md"
							checked={!form.watch('ipv6_disabled')}
							onCheckedChange={(checked) => form.setValue('ipv6_disabled', !checked)}
						/>
					</Setting>

					<Setting
						mini
						title={t('p2p_visibility')}
						description={
							<p className="text-sm text-gray-400">
								{t('p2p_visibility_description')}
							</p>
						}
					>
						<Select
							value={form.watch('discovery') || 'Everyone'}
							containerClassName="h-[30px]"
							className="h-full"
							onChange={(type) => form.setValue('discovery', type)}
						>
							<SelectOption value="Everyone">
								{t('p2p_visibility_everyone')}
							</SelectOption>
							{isP2PWipFeatureEnabled ? (
								<SelectOption value="ContactsOnly">
									{t('p2p_visibility_contacts_only')}
								</SelectOption>
							) : null}
							<SelectOption value="Disabled">
								{t('p2p_visibility_disabled')}
							</SelectOption>
						</Select>
					</Setting>

					<Setting
						mini
						title={t('enable_relay')}
						description={
							<>
								<p className="text-sm text-gray-400">
									{t('enable_relay_description')}
								</p>
							</>
						}
					>
						<Switch
							size="md"
							checked={!form.watch('relay_disabled')}
							onCheckedChange={(checked) => form.setValue('relay_disabled', !checked)}
						/>
					</Setting>

					{isP2PWipFeatureEnabled && (
						<>
							<Setting
								mini
								title={t('remote_access')}
								description={
									<>
										<p className="text-sm text-gray-400">
											{t('remote_access_description')}
										</p>
										<p className="text-sm text-yellow-500">
											WARNING: This protocol has no security at the moment and
											effectively gives root access!
										</p>
									</>
								}
							>
								<Switch
									size="md"
									checked={form.watch('enable_remote_access')}
									onCheckedChange={(checked) =>
										form.setValue('enable_remote_access', checked)
									}
								/>
							</Setting>
						</>
					)}

					<Setting
						mini
						title={t('manual_peers')}
						description={
							<>
								{t('manual_peers_description')
									.split('\n')
									.map((line, index) => (
										<p key={index} className="text-sm text-gray-400">
											{line}
										</p>
									))}
							</>
						}
					></Setting>

					<div className="grid space-y-2">
						{form.watch('p2p_manual_peers')?.map((socket) => (
							<Card key={socket} className="flex justify-between hover:bg-app-box/70">
								<div className="flex">
									<Icon
										size={24}
										name="Node"
										className="mr-3 size-10 self-center"
									/>
									<div className="grid min-w-[110px] grid-cols-1">
										<h1 className="truncate pt-0.5 text-sm font-semibold">
											{socket}
										</h1>
									</div>
								</div>

								<div className="flex items-center">
									<div>
										<Button
											variant="colored"
											className="border-red-500 bg-red-500 focus:ring-1 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-app-selected"
											onClick={() => {
												form.setValue(
													'p2p_manual_peers',
													(
														form.getValues('p2p_manual_peers') || []
													).filter((v) => v !== socket)
												);
											}}
										>
											{t('delete')}
										</Button>
									</div>
								</div>
							</Card>
						))}

						<div className="flex space-x-2">
							<Input
								className="flex-1"
								placeholder="129.168.0.2:1234"
								value={newSocket}
								onChange={(e) => setNewSocket(e.currentTarget.value)}
								onKeyDown={(e) => {
									if (e.key === 'Enter' && !isNewSocketInvalid) {
										form.setValue('p2p_manual_peers', [
											...(form.getValues('p2p_manual_peers') || []),
											newSocket
										]);
										setNewSocket('');
									}
								}}
							/>
							<Button
								variant="outline"
								disabled={isNewSocketInvalid}
								onClick={() => {
									form.setValue('p2p_manual_peers', [
										...(form.getValues('p2p_manual_peers') || []),
										newSocket
									]);
									setNewSocket('');
								}}
							>
								Submit
							</Button>
						</div>
					</div>

					<NodesPanel />
				</>
			) : null}
		</FormProvider>
	);
};

function NodesPanel() {
	const { t } = useLocale();
	const navigate = useNavigate();
	const peers = usePeers();
	const platform = usePlatform();

	const isP2PWipFeatureEnabled = useFeatureFlag('wipP2P');

	const debugConnect = useBridgeMutation(['p2p.debugConnect'], {
		onSuccess: () => toast.success('Connected!'),
		onError: (e) => toast.error(`Error connecting '${e.message}'`)
	});

	return (
		<div className="flex flex-col gap-2">
			<h1 className="text-lg font-bold text-ink">{t('nodes')}</h1>

			{peers.size === 0 ? (
				<p className="text-sm text-gray-400">{t('no_nodes_found')}</p>
			) : (
				<div className="grid grid-cols-1 gap-2">
					{[...peers.entries()].map(([id, peer]) => (
						<Card key={id} className="hover:bg-app-box/70">
							<Icon size={24} name="Node" className="mr-3 size-10 self-center" />
							<div className="grid min-w-[110px] grid-cols-1">
								<Tooltip label={id}>
									<h1 className="truncate pt-0.5 text-sm font-semibold">
										{peer.metadata.name}
									</h1>
								</Tooltip>
								<h2 className="truncate pt-0.5 text-sm font-semibold">
									Spacedrive {peer.metadata.version}{' '}
									{peer.metadata.operating_system
										? `- ${peer.metadata.operating_system}`
										: ''}
								</h2>
							</div>

							<div className="grow"></div>
							<div className="flex items-center justify-center space-x-2">
								{isP2PWipFeatureEnabled && (
									<Button
										onClick={() =>
											platform.confirm(
												'Warning: This will only work if rspc remote is enabled on the remote node and the node is online!',
												(result) => {
													if (result) navigate(`/remote/${id}/browse`);
												}
											)
										}
									>
										rspc remote
									</Button>
								)}

								<Button
									variant="accent"
									onClick={() => debugConnect.mutate(id)}
									disabled={debugConnect.isLoading}
								>
									Connect
								</Button>

								<NodePill>
									{peer.discovery === 'Manual'
										? 'Manual'
										: peer.discovery === 'Local'
											? 'LAN'
											: 'Relay'}
								</NodePill>

								<NodePill
									className={
										peer.connection !== 'Disconnected' ? 'bg-green-400' : ''
									}
								>
									{peer.connection}
								</NodePill>
							</div>
						</Card>
					))}
				</div>
			)}
		</div>
	);
}
