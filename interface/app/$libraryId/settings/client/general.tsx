import { PropsWithChildren } from 'react';
import { FormProvider } from 'react-hook-form';
import {
	ListenerState,
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDebugState,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Switch, Tooltip, tw, z } from '@sd/ui';
import { Icon } from '~/components';
import { useDebouncedFormWatch, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

export const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

function RenderListenerPill(props: PropsWithChildren<{ listener?: ListenerState }>) {
	if (props.listener?.type === 'Error') {
		return (
			<Tooltip label={`Error: ${props.listener.error}`}>
				<NodePill className="bg-red-700">{props.children}</NodePill>
			</Tooltip>
		);
	} else if (props.listener?.type === 'Listening') {
		return <NodePill>{props.children}</NodePill>;
	}
	return null;
}

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const listeners = useBridgeQuery(['p2p.listeners'], {
		refetchInterval: 1000
	});
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');
	const connectedPeers = useConnectedPeers();
	// const image_labeler_versions = useBridgeQuery(['models.image_detection.list']);
	const updateThumbnailerPreferences = useBridgeMutation('nodes.updateThumbnailerPreferences');

	const { t } = useLocale();

	const form = useZodForm({
		schema: z
			.object({
				name: z.string().min(1).max(250).optional(),
				// image_labeler_version: z.string().optional(),
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
			name: node.data?.name
			// image_labeler_version: node.data?.image_labeler_version ?? undefined
			// background_processing_percentage:
			// 	node.data?.preferences.thumbnailer.background_processing_percentage || 50
		}
	});

	// const watchBackgroundProcessingPercentage = form.watch('background_processing_percentage');

	useDebouncedFormWatch(form, async (value) => {
		if (await form.trigger()) {
			await editNode.mutateAsync({
				name: value.name || null,
				p2p_port: null,
				p2p_disabled: null,
				p2p_ipv6_disabled: null,
				p2p_relay_disabled: null,
				p2p_discovery: null,
				p2p_remote_access: null,
				p2p_manual_peers: null
				// image_labeler_version: value.image_labeler_version ?? null
			});

			if (value.background_processing_percentage != null) {
				await updateThumbnailerPreferences.mutateAsync({
					// background_processing_percentage: value.background_processing_percentage
				});
			}
		}

		node.refetch();
	});

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
							<RenderListenerPill listener={listeners.data?.ipv4}>
								IPv4
							</RenderListenerPill>
							<RenderListenerPill listener={listeners.data?.ipv6}>
								IPv6
							</RenderListenerPill>
							<RenderListenerPill listener={listeners.data?.relay}>
								Relay
							</RenderListenerPill>
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
												t('data_folder_modification_warning'),
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
			{/* <Setting
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
			</Setting> */}
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
		</FormProvider>
	);
};
