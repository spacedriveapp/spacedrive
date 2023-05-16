import { useQueryClient } from '@tanstack/react-query';
import { Archive, ArrowsClockwise, Info, Trash } from 'phosphor-react';
import { useState } from 'react';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Divider, Tooltip, forms, tw } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { useZodRouteParams } from '~/hooks';
import ModalLayout from '../../ModalLayout';
import { IndexerRuleEditor } from './IndexerRuleEditor';

const Label = tw.label`mb-1 text-sm font-medium`;
const FlexCol = tw.label`flex flex-col flex-1`;
const InfoText = tw.p`mt-2 text-xs text-ink-faint`;
const ToggleSection = tw.label`flex flex-row w-full`;

const { Form, Input, Switch, useZodForm, z } = forms;

const schema = z.object({
	name: z.string(),
	path: z.string(),
	hidden: z.boolean(),
	indexerRulesIds: z.array(z.number()),
	syncPreviewMedia: z.boolean(),
	generatePreviewMedia: z.boolean()
});

const PARAMS = z.object({
	id: z.coerce.number().default(0)
});

export const Component = () => {
	const form = useZodForm({
		schema,
		defaultValues: {
			indexerRulesIds: []
		}
	});

	const { id: locationId } = useZodRouteParams(PARAMS);

	const navigate = useNavigate();
	const fullRescan = useLibraryMutation('locations.fullRescan');
	const queryClient = useQueryClient();
	const [isFirstLoad, setIsFirstLoad] = useState<boolean>(true);
	const updateLocation = useLibraryMutation('locations.update', {
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'Failed to update location settings'
			});
		},
		onSuccess: () => {
			form.reset(form.getValues());
			queryClient.invalidateQueries(['locations.list']);
		}
	});

	const { isDirty } = form.formState;

	useLibraryQuery(['locations.getById', locationId], {
		onSettled: (data, error) => {
			if (isFirstLoad) {
				// @ts-expect-error // TODO: Fix the types
				if (!data && error == null) error = new Error('Failed to load location settings');

				// Return to previous page when no data is available at first load
				if (error) navigate(-1);
				else setIsFirstLoad(false);
			}

			if (error) {
				showAlertDialog({
					title: 'Error',
					value: 'Failed to load location settings'
				});
			} else if (data && (isFirstLoad || !isDirty)) {
				form.reset({
					path: data.path,
					name: data.name,
					hidden: data.hidden,
					indexerRulesIds: data.indexer_rules.map((i) => i.indexer_rule.id),
					syncPreviewMedia: data.sync_preview_media,
					generatePreviewMedia: data.generate_preview_media
				});
			}
		}
	});

	const onSubmit = form.handleSubmit(
		({ name, hidden, indexerRulesIds, syncPreviewMedia, generatePreviewMedia }) =>
			updateLocation.mutateAsync({
				id: locationId,
				name,
				hidden,
				indexer_rules_ids: indexerRulesIds,
				sync_preview_media: syncPreviewMedia,
				generate_preview_media: generatePreviewMedia
			})
	);

	return (
		<Form form={form} disabled={isFirstLoad} onSubmit={onSubmit} className="h-full w-full">
			<ModalLayout
				title="Edit Location"
				topRight={
					<div className="flex flex-row space-x-3">
						{isDirty && (
							<Button onClick={() => form.reset()} variant="outline" size="sm">
								Reset
							</Button>
						)}
						<Button
							type="submit"
							disabled={!isDirty || form.formState.isSubmitting}
							variant={isDirty ? 'accent' : 'outline'}
							size="sm"
						>
							Save Changes
						</Button>
					</div>
				}
			>
				<div className="flex space-x-4">
					<FlexCol>
						<Input label="Display Name" {...form.register('name')} />
						<InfoText>
							The name of this Location, this is what will be displayed in the
							sidebar. Will not rename the actual folder on disk.
						</InfoText>
					</FlexCol>
					<FlexCol>
						<Input
							label="Local Path"
							readOnly={true}
							className="text-ink-dull"
							{...form.register('path')}
						/>
						<InfoText>
							The path to this Location, this is where the files will be stored on
							disk.
						</InfoText>
					</FlexCol>
				</div>
				<Divider />
				<div className="space-y-2">
					<ToggleSection>
						<Label className="grow">Generate preview media for this Location</Label>
						<Switch {...form.register('generatePreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">
							Sync preview media for this Location with your devices
						</Label>
						<Switch {...form.register('syncPreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="grow">
							Hide location and contents from view{' '}
							<Tooltip label='Prevents the location and its contents from appearing in summary categories, search and tags unless "Show hidden items" is enabled.'>
								<Info className="inline" />
							</Tooltip>
						</Label>
						<Switch {...form.register('hidden')} size="sm" />
					</ToggleSection>
				</div>
				<Divider />
				<div className="flex flex-col">
					<Label className="grow">Indexer rules</Label>
					<InfoText className="mb-1 mt-0">
						Indexer rules allow you to specify paths to ignore using RegEx.
					</InfoText>
					<Controller
						name="indexerRulesIds"
						render={({ field }) => <IndexerRuleEditor field={field} editable />}
						control={form.control}
					/>
				</div>
				<Divider />
				<div className="flex space-x-5">
					<FlexCol>
						<div>
							<Button
								onClick={() => fullRescan.mutate(locationId)}
								size="sm"
								variant="outline"
							>
								<ArrowsClockwise className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Full Reindex
							</Button>
						</div>
						<InfoText>Perform a full rescan of this Location.</InfoText>
					</FlexCol>
					<FlexCol>
						<div>
							<Button
								onClick={() => alert('Archiving locations is coming soon...')}
								size="sm"
								variant="outline"
								className=""
							>
								<Archive className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Archive
							</Button>
						</div>
						<InfoText>
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
							>
								<Trash className="-mt-0.5 mr-1.5 inline h-4 w-4" />
								Delete
							</Button>
						</div>
						<InfoText>
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
