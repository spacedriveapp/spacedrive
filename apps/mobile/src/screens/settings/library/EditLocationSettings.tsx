import { useQueryClient } from '@tanstack/react-query';
import { Archive, ArrowsClockwise, Trash } from 'phosphor-react-native';
import { useEffect } from 'react';
import { Controller } from 'react-hook-form';
import { Alert, Text, View } from 'react-native';
import { z } from 'zod';
import { useLibraryMutation, useLibraryQuery, useZodForm } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { AnimatedButton } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { Input } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
import SettingsButton from '~/components/settings/SettingsButton';
import { SettingsInputInfo, SettingsTitle } from '~/components/settings/SettingsContainer';
import SettingsToggle from '~/components/settings/SettingsToggle';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const schema = z.object({
	displayName: z.string().nullable(),
	path: z.string().min(1).nullable(),
	localPath: z.string().nullable(),
	indexer_rules_ids: z.array(z.string()),
	generatePreviewMedia: z.boolean().nullable(),
	syncPreviewMedia: z.boolean().nullable(),
	hidden: z.boolean().nullable()
});

const EditLocationSettingsScreen = ({
	route,
	navigation
}: SettingsStackScreenProps<'EditLocationSettings'>) => {
	const { id } = route.params;

	const queryClient = useQueryClient();

	const form = useZodForm({ schema });

	const updateLocation = useLibraryMutation('locations.update', {
		onError: (e) => console.log({ e }),
		onSuccess: () => {
			form.reset(form.getValues());
			queryClient.invalidateQueries({ queryKey: ['locations.list'] });
			toast.success('Location updated!');
			// TODO: navigate back & reset input focus!
		}
	});

	const onSubmit = form.handleSubmit((data) =>
		updateLocation.mutateAsync({
			id: Number(id),
			name: data.displayName,
			path: data.path,
			sync_preview_media: data.syncPreviewMedia,
			generate_preview_media: data.generatePreviewMedia,
			hidden: data.hidden,
			indexer_rules_ids: []
		})
	);

	useEffect(() => {
		navigation.setOptions({
			headerRight: () => (
				<View style={tw`mr-1 flex flex-row gap-x-1`}>
					{form.formState.isDirty && (
						<AnimatedButton
							variant="outline"
							onPress={() => form.reset()}
							disabled={!form.formState.isDirty}
						>
							<Text style={tw`text-white`}>Reset</Text>
						</AnimatedButton>
					)}
					<AnimatedButton
						onPress={onSubmit}
						disabled={!form.formState.isDirty || form.formState.isSubmitting}
						variant={form.formState.isDirty ? 'accent' : 'outline'}
					>
						<Text
							style={twStyle(
								'font-medium',
								form.formState.isDirty ? 'text-white' : ' text-ink-faint'
							)}
						>
							Save
						</Text>
					</AnimatedButton>
				</View>
			)
		});
	}, [form, navigation, onSubmit]);

	const query = useLibraryQuery(['locations.getWithRules', id]);
	useEffect(() => {
		const data = query.data;
		if (data && !form.formState.isDirty)
			form.reset({
				displayName: data.name,
				localPath: data.path,
				indexer_rules_ids: data.indexer_rules.map((i) => i.id.toString()),
				generatePreviewMedia: data.generate_preview_media,
				syncPreviewMedia: data.sync_preview_media,
				hidden: data.hidden
			});
	}, [form, query.data]);

	const fullRescan = useLibraryMutation('locations.fullRescan');

	return (
		<ScreenContainer style={tw`px-6`}>
			{/* Inputs */}
			<View>
				<SettingsTitle style={tw`mb-1`}>Display Name</SettingsTitle>
				<Controller
					name="displayName"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value ?? undefined} />
					)}
				/>
				<SettingsInputInfo>
					The name of this Location, this is what will be displayed in the sidebar. Will
					not rename the actual folder on disk.
				</SettingsInputInfo>

				<SettingsTitle style={tw`mb-1 mt-3`}>Local Path</SettingsTitle>
				<Controller
					name="localPath"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value ?? undefined} />
					)}
				/>
				<SettingsInputInfo>
					The path to this Location, this is where the files will be stored on disk.
				</SettingsInputInfo>
			</View>
			<Divider style={tw`my-0`} />
			{/* Switches */}
			<View style={tw`gap-y-6`}>
				<SettingsToggle
					name="generatePreviewMedia"
					control={form.control}
					title="Generate preview media"
				/>
				<SettingsToggle
					control={form.control}
					name="syncPreviewMedia"
					title="Sync preview media with your devices"
				/>
				<SettingsToggle
					control={form.control}
					name="hidden"
					title="Hide location and contents from view"
				/>
			</View>
			{/* Buttons */}
			<View style={tw`gap-y-6`}>
				<SettingsButton
					title="Reindex"
					description="Perform a full rescan of this location"
					onPress={
						() => fullRescan.mutate({ location_id: id, reidentify_objects: false }) //FIXME: The famous serializing error for fullRescan. Keep this false until it's fixed.
					}
					buttonText="Full Reindex"
					buttonIcon={<ArrowsClockwise color="white" size={20} />}
					buttonTextStyle="text-white font-bold"
					buttonVariant="outline"
					infoContainerStyle={'w-[50%]'}
				/>
				<SettingsButton
					title="Archive Location"
					description="Extract data from Library as an archive, useful to preserve Location folder structure."
					buttonText="Archive"
					buttonIcon={<Archive color="white" size={20} />}
					onPress={() => Alert.alert('Archiving locations is coming soon...')}
					buttonVariant="outline"
					buttonTextStyle="text-white font-bold"
					infoContainerStyle={'w-[60%]'}
				/>
				<SettingsButton
					title="Delete Location"
					description="This will not delete the actual folder on disk. Preview media will be...???"
					buttonText="Delete"
					buttonIcon={<Trash color="white" size={20} />}
					onPress={() => Alert.alert('Deleting locations is coming soon...')}
					buttonVariant="danger"
					buttonTextStyle="text-white font-bold"
					infoContainerStyle={'w-[60%]'}
				/>
				{/* Indexer Rules */}
				<Text style={tw`text-center text-xs font-bold text-white`}>
					TODO: Indexer Rules
				</Text>
			</View>
		</ScreenContainer>
	);
};

export default EditLocationSettingsScreen;
