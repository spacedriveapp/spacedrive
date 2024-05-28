import clsx from 'clsx';
import { FormProvider } from 'react-hook-form';
import {
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useFeatureFlag,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Select, SelectOption, Slider, Switch, tw, z } from '@sd/ui';
import { Icon } from '~/components';
import { useDebouncedFormWatch, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

const u16 = () => z.number().min(0).max(65535);

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');
	const connectedPeers = useConnectedPeers();
	// const image_labeler_versions = useBridgeQuery(['models.image_detection.list']);
	const updateThumbnailerPreferences = useBridgeMutation('nodes.updateThumbnailerPreferences');

	const { t } = useLocale();

	console.log(node.data);
	// console.log(node.data?.delete_prompt);

	const form = useZodForm({
		schema: z
			.object({
				name: z.string().min(1).max(250).optional(),
				p2p_port: z.discriminatedUnion('type', [
					z.object({ type: z.literal('random') }),
					z.object({ type: z.literal('discrete'), value: u16() })
				]),
				p2p_ipv4_enabled: z.boolean().optional(),
				p2p_ipv6_enabled: z.boolean().optional(),
				p2p_discovery: z
					.union([
						z.literal('Everyone'),
						z.literal('ContactsOnly'),
						z.literal('Disabled')
					])
					.optional(),
				p2p_remote_access: z.boolean().optional(),
				image_labeler_version: z.string().optional(),
				background_processing_percentage: z.coerce
					.number({
						invalid_type_error: 'Must use numbers from 0 to 100'
					})
					.int()
					.nonnegative()
					.lte(100),
				delete_prompt: z.union([
					z.literal('ShowPrompt'),
					z.literal('SendTrash'),
					z.literal('DeleteInstantly')
				])
			})
			.strict(),
		reValidateMode: 'onChange',
		defaultValues: {
			name: node.data?.name,
			p2p_port: node.data?.p2p.port || { type: 'random' },
			p2p_ipv4_enabled: node.data?.p2p.ipv4 || true,
			p2p_ipv6_enabled: node.data?.p2p.ipv6 || true,
			p2p_discovery: node.data?.p2p.discovery || 'Everyone',
			p2p_remote_access: node.data?.p2p.remote_access || false,
			image_labeler_version: node.data?.image_labeler_version ?? undefined,
			background_processing_percentage:
				node.data?.preferences.thumbnailer.background_processing_percentage || 50,
			delete_prompt: node.data?.delete_prompt.option || 'ShowPrompt'
		}
	});
	const p2p_port = form.watch('p2p_port');

	const watchBackgroundProcessingPercentage = form.watch('background_processing_percentage');

	useDebouncedFormWatch(form, async (value) => {
		if (await form.trigger()) {
			await editNode.mutateAsync({
				name: value.name || null,

				p2p_port: (value.p2p_port as any) ?? null,
				p2p_ipv4_enabled: value.p2p_ipv4_enabled ?? null,
				p2p_ipv6_enabled: value.p2p_ipv6_enabled ?? null,
				p2p_discovery: value.p2p_discovery ?? null,
				p2p_remote_access: value.p2p_remote_access ?? null,
				image_labeler_version: value.image_labeler_version ?? null,
				delete_prompt: value.delete_prompt ?? "ShowPrompt"
			});

			if (value.background_processing_percentage != undefined) {
				await updateThumbnailerPreferences.mutateAsync({
					background_processing_percentage: value.background_processing_percentage
				});
			}
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
				title={t('general_settings')}
				description={t('general_settings_description')}
			/>
			{/* Node Card */}
			<Card className="px-5">
				<div className="my-2 flex w-full flex-col">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">{t('local_node')}</span>
						<div className="flex flex-row space-x-1">
							<NodePill>
								{connectedPeers.size} {t('peers')}
							</NodePill>
							{/* {node.data?.p2p_enabled === true ? (
								<NodePill className="!bg-accent text-white">
									{t('running')}
								</NodePill>
							) : (
								<NodePill className="text-white">{t('disabled')}</NodePill>
							)} */}
						</div>
					</div>

					<hr className="mb-4 mt-2 flex w-full border-app-line" />
					<div className="flex w-full items-center gap-5">
						<Icon name="Laptop" className="mt-2 size-14" />
						<div className="flex flex-col">
							<NodeSettingLabel>{t('node_name')}</NodeSettingLabel>
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
							<NodeSettingLabel>{t('data_folder')}</NodeSettingLabel>
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
									{t('open')}
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
			{/* Debug Mode */}
			<Setting mini title={t('debug_mode')} description={t('debug_mode_description')}>
				<Switch
					size="md"
					checked={debugState.enabled}
					onClick={() => (debugState.enabled = !debugState.enabled)}
				/>
			</Setting>
			{/* Background Processing */}
			<Setting
				mini
				registerName="background_processing_percentage"
				title={t('thumbnailer_cpu_usage')}
				description={t('thumbnailer_cpu_usage_description')}
			>
				<div className="flex h-[30px] w-80 items-center gap-2">
					<Slider
						onValueChange={(value) => {
							if (value.length > 0) {
								form.setValue('background_processing_percentage', value[0] ?? 0);
							}
						}}
						max={100}
						step={25}
						min={0}
						value={[watchBackgroundProcessingPercentage]}
					/>
					<Input
						className="after:h-initial relative h-[30px] w-[8ch]
						after:absolute after:right-[0.8em] after:top-1/2 after:inline-block after:-translate-y-2/4 after:content-['%']"
						defaultValue={
							node.data?.preferences.thumbnailer.background_processing_percentage ||
							75
						}
						maxLength={3}
						{...form.register('background_processing_percentage', {
							valueAsNumber: true
						})}
					/>
				</div>
			</Setting>
			{/* Image Labeler */}
			{/* <Setting
				mini
				title={t('image_labeler_ai_model')}
				description={t('image_labeler_ai_model_description')}
				registerName="image_labeler_version"
			>
				<div className="flex h-[30px]">
					<Controller
						name="image_labeler_version"
						disabled={node.data?.image_labeler_version == null}
						control={form.control}
						render={({ field }) => (
							<Select {...field} containerClassName="h-[30px] whitespace-nowrap">
								{image_labeler_versions.data?.map((model, key) => (
									<SelectOption key={key} value={model}>
										{model}
									</SelectOption>
								))}
							</Select>
						)}
					/>
				</div>
			</Setting> */}
			<div className="flex flex-col gap-4">
				<h1 className="mb-3 text-lg font-bold text-ink">{t('networking')}</h1>

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
												WARNING: This protocol has no security at the moment
												and effectively gives root access!
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
			</div>
			<div className="flex flex-col gap-4">
				<h1 className="mb-3 text-lg font-bold text-ink">{t('delete_settings')}</h1>

				<Setting
					mini
					title={t('delete_show_prompt')}
					description={
						<p className="text-sm text-gray-400">
							{t('delete_show_prompt_description')}
						</p>
					}
				>
					<Select
						value={form.watch('delete_prompt') || 'ShowPrompt'}
						containerClassName="h-[30px]"
						className="h-full"
						onChange={(type) => form.setValue('delete_prompt', type)}
					>
						<SelectOption value="ShowPrompt">{'Show Prompt'}</SelectOption>
						<SelectOption value="SendTrash">{'Send to Trash'}</SelectOption>
						<SelectOption value="DeleteInstantly">{'Delete Instantly'}</SelectOption>
					</Select>
				</Setting>
			</div>
		</FormProvider>
	);
};
