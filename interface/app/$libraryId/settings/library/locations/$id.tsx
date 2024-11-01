import { Archive, ArrowsClockwise, Info, Trash } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import { Suspense } from 'react';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { useLibraryMutation, useLibraryQuery, useZodForm } from '@sd/client';
import {
	Button,
	dialogManager,
	Divider,
	Form,
	InfoText,
	InputField,
	Label,
	RadioGroupField,
	SwitchField,
	toast,
	Tooltip,
	tw,
	z
} from '@sd/ui';
import ModalLayout from '~/app/$libraryId/settings/ModalLayout';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { useLocale, useZodRouteParams } from '~/hooks';

import DeleteDialog from './DeleteDialog';
import IndexerRuleEditor from './IndexerRuleEditor';
import { LocationPathInputField } from './PathInput';

const FlexCol = tw.label`flex flex-col flex-1`;
const ToggleSection = tw.label`flex flex-row w-full`;

const schema = z.object({
	name: z.string().min(1).nullable(),
	path: z.string().min(1).nullable(),
	hidden: z.boolean().nullable(),
	indexerRulesIds: z.array(z.number()),
	locationType: z.string(),
	syncPreviewMedia: z.boolean().nullable(),
	generatePreviewMedia: z.boolean().nullable()
});

export const Component = () => {
	return (
		<Suspense fallback={<div></div>}>
			<EditLocationForm />
		</Suspense>
	);
};

const EditLocationForm = () => {
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const navigate = useNavigate();
	const fullRescan = useLibraryMutation('locations.fullRescan');
	const queryClient = useQueryClient();

	const locationDataQuery = useLibraryQuery(['locations.getWithRules', locationId], {
		suspense: true
	});
	const locationData = locationDataQuery.data;

	const form = useZodForm({
		schema,
		defaultValues: {
			indexerRulesIds: locationData?.indexer_rules.map((rule) => rule.id) ?? [],
			locationType: 'normal',
			name: locationData?.name ?? '',
			path: locationData?.path ?? '',
			hidden: locationData?.hidden ?? false,
			syncPreviewMedia: locationData?.sync_preview_media ?? false,
			generatePreviewMedia: locationData?.generate_preview_media ?? false
		}
	});

	const updateLocation = useLibraryMutation('locations.update', {
		onError: () => {
			toast.error(t('failed_to_update_location_settings'));
		},
		onSuccess: () => {
			form.reset(form.getValues());
			queryClient.invalidateQueries({ queryKey: ['locations.list'] });
		}
	});

	const onSubmit = form.handleSubmit((data) =>
		updateLocation.mutateAsync({
			id: locationId,
			path: data.path,
			name: data.name,
			hidden: data.hidden,
			indexer_rules_ids: data.indexerRulesIds,
			sync_preview_media: data.syncPreviewMedia,
			generate_preview_media: data.generatePreviewMedia
		})
	);

	const { t } = useLocale();

	return (
		<Form form={form} onSubmit={onSubmit} className="size-full">
			<ModalLayout
				title={t('edit_location')}
				topRight={
					<div className="flex flex-row space-x-3">
						{form.formState.isDirty && (
							<Button onClick={() => form.reset()} variant="outline" size="sm">
								{t('reset')}
							</Button>
						)}
						<Button
							type="submit"
							disabled={!form.formState.isDirty || form.formState.isSubmitting}
							variant={form.formState.isDirty ? 'accent' : 'outline'}
							size="sm"
						>
							{t('save_changes')}
						</Button>
					</div>
				}
			>
				<div className="flex space-x-4">
					<FlexCol>
						<InputField label={t('display_name')} {...form.register('name')} />
						<InfoText className="mt-2">{t('location_display_name_info')}</InfoText>
					</FlexCol>
					<FlexCol>
						<LocationPathInputField label={t('path')} {...form.register('path')} />
						<InfoText className="mt-2">{t('location_path_info')}</InfoText>
					</FlexCol>
				</div>
				<Divider />
				<div className="space-y-2">
					<Label className="grow">{t('location_type')}</Label>
					<RadioGroupField.Root
						className="flex flex-row !space-y-0 space-x-2"
						{...form.register('locationType')}
					>
						<RadioGroupField.Item key="normal" value="normal">
							<h1 className="font-bold">{t('normal')}</h1>
							<p className="text-sm text-ink-faint">{t('location_type_normal')}</p>
						</RadioGroupField.Item>

						<RadioGroupField.Item disabled key="managed" value="managed">
							<h1 className="font-bold">{t('managed')}</h1>
							<p className="text-sm text-ink-faint">{t('location_type_managed')}</p>
						</RadioGroupField.Item>

						<RadioGroupField.Item disabled key="replica" value="replica">
							<h1 className="font-bold">{t('replica')}</h1>
							<p className="text-sm text-ink-faint">{t('location_type_replica')}</p>
						</RadioGroupField.Item>
					</RadioGroupField.Root>
				</div>
				<Divider />
				<div className="space-y-2">
					<ToggleSection>
						<Label className="grow">{t('generate_preview_media_label')}</Label>
						<SwitchField {...form.register('generatePreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">{t('sync_preview_media_label')}</Label>
						<SwitchField {...form.register('syncPreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">
							{t('hide_location_from_view')}{' '}
							<Tooltip label={t('hidden_label')}>
								<Info className="inline" />
							</Tooltip>
						</Label>
						<SwitchField {...form.register('hidden')} size="sm" />
					</ToggleSection>
				</div>
				<Divider />
				<Controller
					name="indexerRulesIds"
					render={({ field }) => (
						<IndexerRuleEditor
							field={field}
							label={t('indexer_rules')}
							editable={true}
							infoText={t('indexer_rules_info')}
							className="flex flex-col rounded-md border border-app-line bg-app-overlay p-5"
						/>
					)}
					control={form.control}
				/>
				<Divider />
				<div className="flex space-x-5">
					<FlexCol>
						<div>
							<Button
								onClick={() =>
									fullRescan.mutate({
										location_id: locationId,
										reidentify_objects: true
									})
								}
								size="sm"
								variant="outline"
							>
								<ArrowsClockwise className="-mt-0.5 mr-1.5 inline size-4" />
								{t('full_reindex')}
							</Button>
						</div>
						<InfoText className="mt-2">{t('full_reindex_info')}</InfoText>
					</FlexCol>
					<FlexCol>
						<div>
							<Button
								onClick={() => toast.info(t('archive_coming_soon'))}
								size="sm"
								variant="outline"
							>
								<Archive className="-mt-0.5 mr-1.5 inline size-4" />
								{t('archive')}
							</Button>
						</div>
						<InfoText className="mt-2">{t('archive_info')}</InfoText>
					</FlexCol>
					<FlexCol>
						<div>
							<Button
								size="sm"
								variant="colored"
								className="border-red-500 bg-red-500"
								onClick={(e: { stopPropagation: () => void }) => {
									e.stopPropagation();
									dialogManager.create((dp) => (
										<DeleteDialog
											{...dp}
											onSuccess={() => navigate(-1)}
											locationId={locationId}
										/>
									));
								}}
							>
								<Trash className="-mt-0.5 mr-1.5 inline size-4" />
								{t('delete')}
							</Button>
						</div>
						<InfoText className="mt-2">{t('delete_info')}</InfoText>
					</FlexCol>
				</div>
				<Divider />
				<div className="h-6" />
			</ModalLayout>
		</Form>
	);
};
