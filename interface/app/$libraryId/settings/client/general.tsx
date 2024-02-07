import clsx from 'clsx';
import { Controller, FormProvider } from 'react-hook-form';
import {
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Select, SelectOption, Slider, Switch, tw, z } from '@sd/ui';
import i18n from '~/app/I18n';
import { Icon } from '~/components';
import { useDebouncedFormWatch, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

// https://doc.rust-lang.org/std/u16/index.html
const u16 = z.number().min(0).max(65_535);

const LANGUAGE_OPTIONS = [
	{ value: 'en', label: 'English' },
	{ value: 'de', label: 'Deutsch' },
	{ value: 'es', label: 'Español' },
	{ value: 'fr', label: 'Français' },
	{ value: 'tr', label: 'Türkçe' },
	{ value: 'nl', label: 'Nederlands'},
	{ value: 'zh-CN', label: '中文（简体）' },
	{ value: 'zh-TW', label: '中文（繁體）' },
	{ value: 'it', label: "Italiano"}
];

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');
	const connectedPeers = useConnectedPeers();
	const image_labeler_versions = useBridgeQuery(['models.image_detection.list']);
	const updateThumbnailerPreferences = useBridgeMutation('nodes.updateThumbnailerPreferences');

	const form = useZodForm({
		schema: z
			.object({
				name: z.string().min(1).max(250).optional(),
				p2p_enabled: z.boolean().optional(),
				p2p_port: u16,
				customOrDefault: z.enum(['Custom', 'Default']),
				image_labeler_version: z.string().optional(),
				background_processing_percentage: z.coerce
					.number({
						invalid_type_error: 'Must use numbers from 0 to 100'
					})
					.int()
					.nonnegative()
					.lte(100)
			})
			.strict(),
		reValidateMode: 'onChange',
		defaultValues: {
			name: node.data?.name,
			p2p_port: node.data?.p2p_port || 0,
			p2p_enabled: node.data?.p2p_enabled,
			customOrDefault: node.data?.p2p_port ? 'Custom' : 'Default',
			image_labeler_version: node.data?.image_labeler_version ?? undefined,
			background_processing_percentage:
				node.data?.preferences.thumbnailer.background_processing_percentage || 50
		}
	});

	const watchCustomOrDefault = form.watch('customOrDefault');
	const watchP2pEnabled = form.watch('p2p_enabled');
	const watchBackgroundProcessingPercentage = form.watch('background_processing_percentage');

	useDebouncedFormWatch(form, async (value) => {
		if (await form.trigger()) {
			await editNode.mutateAsync({
				name: value.name || null,
				p2p_port: value.customOrDefault === 'Default' ? 0 : Number(value.p2p_port),
				p2p_enabled: value.p2p_enabled ?? null,
				image_labeler_version: value.image_labeler_version ?? null
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
		if (Number(data.p2p_port) > 65535) {
			form.setValue('p2p_port', 65535);
		}
	});

	const { t } = useLocale();

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
							{node.data?.p2p_enabled === true ? (
								<NodePill className="!bg-accent text-white">
									{t('running')}
								</NodePill>
							) : (
								<NodePill className="text-white">{t('disabled')}</NodePill>
							)}
						</div>
					</div>

					<hr className="mb-4 mt-2 flex w-full border-app-line" />
					<div className="flex w-full items-center gap-5">
						<Icon name="Laptop" className="mt-2 h-14 w-14" />
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
			{/* Language Settings */}
			<Setting mini title={t('language')} description={t('language_description')}>
				<div className="flex h-[30px] gap-2">
					<Select
						value={i18n.language}
						onChange={(e) => {
							i18n.changeLanguage(e);
							// add "i18nextLng" key to localStorage and set it to the selected language
							localStorage.setItem('i18nextLng', e);
						}}
						containerClassName="h-[30px] whitespace-nowrap"
					>
						{LANGUAGE_OPTIONS.map((lang, key) => (
							<SelectOption key={key} value={lang.value}>
								{lang.label}
							</SelectOption>
						))}
					</Select>
				</div>
			</Setting>
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
			<Setting
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
			</Setting>
			<div className="flex flex-col gap-4">
				<h1 className="mb-3 text-lg font-bold text-ink">{t('networking')}</h1>

				{/* TODO: Add some UI for this stuff */}
				{/* {node.data?.p2p.ipv4.status === 'Listening' ||
				node.data?.p2p.ipv4.status === 'Enabling'
					? `0.0.0.0:${node.data?.p2p.ipv4?.port || 0}`
					: ''}
				{node.data?.p2p.ipv6.status === 'Listening' ||
				node.data?.p2p.ipv6.status === 'Enabling'
					? `[::1]:${node.data?.p2p.ipv6?.port || 0}`
					: ''} */}

				<Setting
					mini
					title={t('enable_networking')}
					// TODO: i18n
					description={
						<>
							<p className="text-sm text-gray-400">
								Allow your node to communicate with other Spacedrive nodes around
								you
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
						onClick={() => form.setValue('p2p_enabled', !form.getValues('p2p_enabled'))}
					/>
				</Setting>
				<Setting
					mini
					title={t('networking_port')}
					description={t('networking_port_description')}
				>
					<div className="flex h-[30px] gap-2">
						<Controller
							control={form.control}
							name="customOrDefault"
							render={({ field }) => (
								<Select
									containerClassName="h-[30px]"
									disabled={!watchP2pEnabled}
									className={clsx(!watchP2pEnabled && 'opacity-50', 'h-full')}
									{...field}
									onChange={(e) => {
										field.onChange(e);
										form.setValue('p2p_port', 0);
									}}
								>
									<SelectOption value="Default">{t('default')}</SelectOption>
									<SelectOption value="Custom">{t('custom')}</SelectOption>
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
		</FormProvider>
	);
};
