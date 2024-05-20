import clsx from 'clsx';
import { FormProvider } from 'react-hook-form';
import { z } from 'zod';
import {
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useFeatureFlag,
	useZodForm
} from '@sd/client';
import { Input, Select, SelectOption, Switch, Tooltip } from '@sd/ui';
import { useDebouncedFormWatch, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

const u16 = () => z.number().min(0).max(65535);

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const listeners = useBridgeQuery(['p2p.listeners'], {
		refetchInterval: 1000
	});
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');
	const connectedPeers = useConnectedPeers();

	const { t } = useLocale();

	const form = useZodForm({
		schema: z
			.object({
				p2p_port: z.discriminatedUnion('type', [
					z.object({ type: z.literal('random') }),
					z.object({ type: z.literal('discrete'), value: u16() })
				]),
				p2p_ipv4_enabled: z.boolean().optional(),
				p2p_ipv6_enabled: z.boolean().optional(),
				p2p_relay_enabled: z.boolean().optional(),
				p2p_discovery: z
					.union([
						z.literal('Everyone'),
						z.literal('ContactsOnly'),
						z.literal('Disabled')
					])
					.optional(),
				p2p_remote_access: z.boolean().optional()
			})
			.strict(),
		reValidateMode: 'onChange',
		defaultValues: {
			p2p_port: node.data?.p2p.port || { type: 'random' },
			p2p_ipv4_enabled: node.data?.p2p.ipv4 || true,
			p2p_ipv6_enabled: node.data?.p2p.ipv6 || true,
			p2p_relay_enabled: node.data?.p2p.relay || true,
			p2p_discovery: node.data?.p2p.discovery || 'Everyone',
			p2p_remote_access: node.data?.p2p.remote_access || false
		}
	});
	const p2p_port = form.watch('p2p_port');

	useDebouncedFormWatch(form, async (value) => {
		if (await form.trigger()) {
			await editNode.mutateAsync({
				name: null,
				p2p_port: (value.p2p_port as any) ?? null,
				p2p_ipv4_enabled: value.p2p_ipv4_enabled ?? null,
				p2p_ipv6_enabled: value.p2p_ipv6_enabled ?? null,
				p2p_relay_enabled: value.p2p_relay_enabled ?? null,
				p2p_discovery: value.p2p_discovery ?? null,
				p2p_remote_access: value.p2p_remote_access ?? null,
				image_labeler_version: null
			});
		}

		node.refetch();
	});

	form.watch((data) => {
		if (data.p2p_port?.type == 'discrete' && Number(data.p2p_port.value) > 65535) {
			form.setValue('p2p_port', { type: 'discrete', value: 65535 });
		}
	});

	const isP2PWipFeatureEnabled = useFeatureFlag('wipP2P');

	return (
		<FormProvider {...form}>
			<Heading
				title={t('network_settings')}
				description={t('network_settings_description')}
			/>

			{/* TODO: Card */}
			{/* TODO: Show node name, name remote identity, etc */}

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
					checked={form.watch('p2p_ipv4_enabled') && form.watch('p2p_ipv6_enabled')}
					onCheckedChange={(checked) => {
						form.setValue('p2p_ipv4_enabled', checked);
						form.setValue('p2p_ipv6_enabled', checked);
					}}
				/>
			</Setting>

			{form.watch('p2p_ipv4_enabled') && form.watch('p2p_ipv6_enabled') ? (
				<>
					<Setting
						mini
						title={t('networking_port')}
						description={t('networking_port_description')}
					>
						<div className="flex h-[30px] gap-2">
							<Select
								value={p2p_port.type}
								containerClassName="h-[30px]"
								className="h-full"
								onChange={(type) => {
									form.setValue('p2p_port', {
										type: type as any
									});
								}}
							>
								<SelectOption value="random">{t('random')}</SelectOption>
								<SelectOption value="discrete">{t('custom')}</SelectOption>
							</Select>
							<Input
								value={p2p_port.type === 'discrete' ? p2p_port.value : 0}
								className={clsx(
									'w-[66px]',
									p2p_port.type === 'random' ? 'opacity-50' : 'opacity-100'
								)}
								disabled={p2p_port.type === 'random'}
								onChange={(e) => {
									form.setValue('p2p_port', {
										type: 'discrete',
										value: Number(e.target.value.replace(/[^0-9]/g, ''))
									});
								}}
							/>
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
							checked={form.watch('p2p_ipv6_enabled')}
							onCheckedChange={(checked) =>
								form.setValue('p2p_ipv6_enabled', checked)
							}
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
							value={form.watch('p2p_discovery') || 'Everyone'}
							containerClassName="h-[30px]"
							className="h-full"
							onChange={(type) => form.setValue('p2p_discovery', type)}
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
							checked={form.watch('p2p_relay_enabled')}
							onCheckedChange={(checked) =>
								form.setValue('p2p_relay_enabled', checked)
							}
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
									checked={form.watch('p2p_remote_access')}
									onCheckedChange={(checked) =>
										form.setValue('p2p_remote_access', checked)
									}
								/>
							</Setting>
						</>
					)}
				</>
			) : null}

			{/* TODO: Allow expanding NLM debug info */}
		</FormProvider>
	);
};
