import { Archive, ArrowsClockwise, Info, Trash } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import { Suspense } from 'react';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { useCache, useLibraryMutation, useLibraryQuery, useNodes, useZodForm } from '@sd/client';
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
import { useZodRouteParams } from '~/hooks';

import DeleteDialog from './DeleteDialog';
import IndexerRuleEditor from './IndexerRuleEditor';
import { LocationPathInputField } from './PathInput';

const FlexCol = tw.label`flex flex-col flex-1`;
const ToggleSection = tw.label`flex flex-row w-full`;

const schema = z.object({
	name: z.string().nullable(),
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
	useNodes(locationDataQuery.data?.nodes);
	const locationData = useCache(locationDataQuery.data?.item);

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
			toast.error('Failed to update location settings');
		},
		onSuccess: () => {
			form.reset(form.getValues());
			queryClient.invalidateQueries(['locations.list']);
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

	return (
		<Form form={form} onSubmit={onSubmit} className="h-full w-full">
			<ModalLayout
				title="Edit Location"
				topRight={
					<div className="flex flex-row space-x-3">
						{form.formState.isDirty && (
							<Button onClick={() => form.reset()} variant="outline" size="sm">
								Reset
							</Button>
						)}
						<Button
							type="submit"
							disabled={!form.formState.isDirty || form.formState.isSubmitting}
							variant={form.formState.isDirty ? 'accent' : 'outline'}
							size="sm"
						>
							Save Changes
						</Button>
					</div>
				}
			>
				<div className="flex space-x-4">
					<FlexCol>
						<InputField label="Display Name" {...form.register('name')} />
						<InfoText className="mt-2">
							The name of this Location, this is what will be displayed in the
							sidebar. Will not rename the actual folder on disk.
						</InfoText>
					</FlexCol>
					<FlexCol>
						<LocationPathInputField label="Path" {...form.register('path')} />
						<InfoText className="mt-2">
							The path to this Location, this is where the files will be stored on
							disk.
						</InfoText>
					</FlexCol>
				</div>
				<Divider />
				<div className="space-y-2">
					<Label className="grow">Location Type</Label>
					<RadioGroupField.Root
						className="flex flex-row !space-y-0 space-x-2"
						{...form.register('locationType')}
					>
						<RadioGroupField.Item key="normal" value="normal">
							<h1 className="font-bold">Normal</h1>
							<p className="text-sm text-ink-faint">
								Contents will be indexed as-is, new files will not be automatically
								sorted.
							</p>
						</RadioGroupField.Item>

						<RadioGroupField.Item disabled key="managed" value="managed">
							<h1 className="font-bold">Managed</h1>
							<p className="text-sm text-ink-faint">
								Spacedrive will sort files for you. If Location isn't empty a
								"spacedrive" folder will be created.
							</p>
						</RadioGroupField.Item>

						<RadioGroupField.Item disabled key="replica" value="replica">
							<h1 className="font-bold">Replica</h1>
							<p className="text-sm text-ink-faint ">
								This Location is a replica of another, its contents will be
								automatically synchronized.
							</p>
						</RadioGroupField.Item>
					</RadioGroupField.Root>
				</div>
				<Divider />
				<div className="space-y-2">
					<ToggleSection>
						<Label className="grow">Generate preview media for this Location</Label>
						<SwitchField {...form.register('generatePreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">
							Sync preview media for this Location with your devices
						</Label>
						<SwitchField {...form.register('syncPreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">
							Hide location and contents from view{' '}
							<Tooltip label='Prevents the location and its contents from appearing in summary categories, search and tags unless "Show hidden items" is enabled.'>
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
							label="Indexer rules"
							editable={true}
							infoText="Indexer rules allow you to specify paths to ignore using globs."
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
								<ArrowsClockwise className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Full Reindex
							</Button>
						</div>
						<InfoText className="mt-2">
							Perform a full rescan of this Location.
						</InfoText>
					</FlexCol>
					<FlexCol>
						<div>
							<Button
								onClick={() => toast.info('Archiving locations is coming soon...')}
								size="sm"
								variant="outline"
								className=""
							>
								<Archive className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Archive
							</Button>
						</div>
						<InfoText className="mt-2">
							Extract data from Library as an archive, useful to preserve Location
							folder structure.
						</InfoText>
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
								<Trash className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Delete
							</Button>
						</div>
						<InfoText className="mt-2">
							This will not delete the actual folder on disk. Preview media will be
						</InfoText>
					</FlexCol>
				</div>
				<Divider />
				<div className="h-6" />
			</ModalLayout>
		</Form>
	);
};
