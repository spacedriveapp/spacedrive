import { useQueryClient } from '@tanstack/react-query';
import { Archive, ArrowsClockwise, Trash } from 'phosphor-react-native';
import { useEffect } from 'react';
import { Controller } from 'react-hook-form';
import { Alert, ScrollView, Text, View } from 'react-native';
import { z } from 'zod';
import { useLibraryMutation, useLibraryQuery, useZodForm } from '@sd/client';
import { Input } from '~/components/form/Input';
import { Switch } from '~/components/form/Switch';
import DeleteLocationModal from '~/components/modal/confirmModals/DeleteLocationModal';
import { AnimatedButton, FakeButton } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import {
	SettingsContainer,
	SettingsInputInfo,
	SettingsTitle
} from '~/components/settings/SettingsContainer';
import { SettingsItem } from '~/components/settings/SettingsItem';
import { tw, twStyle } from '~/lib/tailwind';
import { type SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const schema = z.object({
	displayName: z.string().nullable(),
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
			queryClient.invalidateQueries(['locations.list']);
			// TODO: Show toast & navigate back & reset input focus!
		}
	});

	const onSubmit = form.handleSubmit((data) =>
		updateLocation.mutateAsync({
			id: Number(id),
			name: data.displayName,
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

	useLibraryQuery(['locations.getWithRules', id], {
		onSuccess: (data) => {
			if (data && !form.formState.isDirty)
				form.reset({
					displayName: data.name,
					localPath: data.path,
					indexer_rules_ids: data.indexer_rules.map((i) => i.indexer_rule.id.toString()),
					generatePreviewMedia: data.generate_preview_media,
					syncPreviewMedia: data.sync_preview_media,
					hidden: data.hidden
				});
		}
	});

	const fullRescan = useLibraryMutation('locations.fullRescan');

	return (
		<ScrollView contentContainerStyle={tw`gap-y-6 pb-12 pt-4`}>
			{/* Inputs */}
			<View style={tw`px-2`}>
				<SettingsTitle>Display Name</SettingsTitle>
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

				<SettingsTitle style={tw`mt-3`}>Local Path</SettingsTitle>
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
				<SettingsContainer>
					<SettingsItem
						title="Generate preview media"
						rightArea={
							<Controller
								name="generatePreviewMedia"
								control={form.control}
								render={({ field: { onChange, value } }) => (
									<Switch value={value ?? undefined} onValueChange={onChange} />
								)}
							/>
						}
					/>
					<SettingsItem
						title="Sync preview media with your devices"
						rightArea={
							<Controller
								name="syncPreviewMedia"
								control={form.control}
								render={({ field: { onChange, value } }) => (
									<Switch value={value ?? undefined} onValueChange={onChange} />
								)}
							/>
						}
					/>
					<SettingsItem
						title="Hide location and contents from view"
						rightArea={
							<Controller
								name="hidden"
								control={form.control}
								render={({ field: { onChange, value } }) => (
									<Switch value={value ?? undefined} onValueChange={onChange} />
								)}
							/>
						}
					/>
				</SettingsContainer>
			</View>
			{/* Indexer Rules */}
			<Text style={tw`text-center text-xs font-bold text-white`}>TODO: Indexer Rules</Text>
			{/* Buttons */}
			<View style={tw`gap-y-6`}>
				<SettingsContainer description="Perform a full rescan of this Location.">
					<SettingsItem
						title="Full Reindex"
						rightArea={
							<AnimatedButton
								size="sm"
								onPress={() =>
									fullRescan.mutate({ location_id: id, reidentify_objects: true })
								}
							>
								<ArrowsClockwise color="white" size={20} />
							</AnimatedButton>
						}
					/>
				</SettingsContainer>
				<SettingsContainer description="Extract data from Library as an archive, useful to preserve Location folder structure.">
					<SettingsItem
						title="Archive"
						rightArea={
							<AnimatedButton
								size="sm"
								onPress={() => Alert.alert('Archiving locations is coming soon...')}
							>
								<Archive color="white" size={20} />
							</AnimatedButton>
						}
					/>
				</SettingsContainer>
				<SettingsContainer description="This will not delete the actual folder on disk. Preview media will be...???">
					<SettingsItem
						title="Delete"
						rightArea={
							<DeleteLocationModal
								locationId={id}
								trigger={
									<FakeButton size="sm" variant="danger">
										<Trash color={tw.color('ink')} size={20} />
									</FakeButton>
								}
							/>
						}
					/>
				</SettingsContainer>
			</View>
		</ScrollView>
	);
};

export default EditLocationSettingsScreen;
