import { Archive, ArrowsClockwise, Info, Trash } from 'phosphor-react';
import { useFormState } from 'react-hook-form';
import { useParams } from 'react-router';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, TextArea, forms, tw } from '@sd/ui';
import { Divider } from '~/components/explorer/inspector/Divider';
import { SettingsSubPage } from '~/components/settings/SettingsSubPage';
import { Tooltip } from '~/components/tooltip/Tooltip';
import { IndexerRuleEditor } from './IndexerRuleEditor';

const InfoText = tw.p`mt-2 text-xs text-ink-faint`;
const Label = tw.label`mb-1 text-sm font-medium`;
const FlexCol = tw.label`flex flex-col flex-1`;
const ToggleSection = tw.label`flex flex-row w-full`;

const { Form, Input, Switch, useZodForm, z } = forms;

export type EditLocationParams = {
	id: string;
};

const schema = z.object({
	displayName: z.string(),
	localPath: z.string(),
	indexer_rules_ids: z.array(z.string()),
	generatePreviewMedia: z.boolean(),
	syncPreviewMedia: z.boolean(),
	hidden: z.boolean()
});

export default function EditLocation() {
	const { id } = useParams<keyof EditLocationParams>() as EditLocationParams;

	useLibraryQuery(['locations.getById', Number(id)], {
		onSuccess: (data) => {
			if (data && !isDirty)
				form.reset({
					displayName: data.name || undefined,
					localPath: data.local_path || undefined,
					indexer_rules_ids: data.indexer_rules.map((i) => i.indexer_rule_id.toString()),
					generatePreviewMedia: data.generate_preview_media,
					syncPreviewMedia: data.sync_preview_media,
					hidden: data.hidden
				});
		}
	});

	const form = useZodForm({
		schema
	});

	const updateLocation = useLibraryMutation('locations.update', {
		onError: (e) => console.log({ e }),
		onSuccess: (e) => form.reset(form.getValues())
	});

	const onSubmit = form.handleSubmit(async (data) => {
		console.log({ data });
		updateLocation.mutate({
			id: Number(id),
			name: data.displayName,
			sync_preview_media: data.syncPreviewMedia,
			generate_preview_media: data.generatePreviewMedia,
			hidden: data.hidden,
			indexer_rules_ids: []
		});
	});

	const fullRescan = useLibraryMutation('locations.fullRescan');

	const { isDirty } = useFormState({ control: form.control });

	return (
		<Form form={form} onSubmit={onSubmit}>
			<SettingsSubPage
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
				{/* {JSON.stringify(form.formState.errors)} */}

				<div className="flex space-x-4">
					<FlexCol>
						<Label>Display Name</Label>
						<Input {...form.register('displayName')} />
						<InfoText>
							The name of this Location, this is what will be displayed in the sidebar. Will not
							rename the actual folder on disk.
						</InfoText>
					</FlexCol>
					<FlexCol>
						<Label>Local Path</Label>
						<Input {...form.register('localPath')} />
						<InfoText>
							The path to this Location, this is where the files will be stored on disk.
						</InfoText>
					</FlexCol>
				</div>
				<Divider />

				<div className="space-y-2">
					<ToggleSection>
						<Label className="flex-grow">Generate preview media for this Location</Label>
						<Switch {...form.register('generatePreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="flex-grow">
							Sync preview media for this Location with your devices
						</Label>
						<Switch {...form.register('syncPreviewMedia')} size="sm" />
					</ToggleSection>
					<ToggleSection>
						<Label className="flex-grow">
							Hide location and contents from view{' '}
							<Tooltip label='Prevents the location and its contents from appearing in summary categories, search and tags unless "Show hidden items" is enabled.'>
								<Info className="inline" />
							</Tooltip>
						</Label>
						<Switch {...form.register('hidden')} size="sm" />
					</ToggleSection>
				</div>
				<Divider />
				<div className="flex flex-col pointer-events-none opacity-30">
					<Label className="flex-grow">Indexer rules</Label>
					<InfoText className="mt-0 mb-1">
						Indexer rules allow you to specify paths to ignore using RegEx.
					</InfoText>
					<IndexerRuleEditor locationId={id} />
				</div>
				<Divider />
				<div className="flex space-x-5">
					<FlexCol>
						<div>
							<Button onClick={() => fullRescan.mutate(Number(id))} size="sm" variant="outline">
								<ArrowsClockwise className="inline w-4 h-4 mr-1.5 -mt-0.5" />
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
								<Archive className="inline w-4 h-4 mr-1.5 -mt-0.5" />
								Archive
							</Button>
						</div>
						<InfoText>
							Extract data from Library as an archive, useful to preserve Location folder structure.
						</InfoText>
					</FlexCol>
					<FlexCol>
						<div>
							<Button size="sm" variant="colored" className="bg-red-500 border-red-500 ">
								<Trash className="inline w-4 h-4 mr-1.5 -mt-0.5" />
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
			</SettingsSubPage>
		</Form>
	);
}
