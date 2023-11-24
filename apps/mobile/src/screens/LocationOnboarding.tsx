import React, { useCallback, useEffect } from 'react';
import { tw, twStyle } from '~/lib/tailwind';
import { Platform, Pressable, Text, ToastAndroid, View } from 'react-native';
import FolderIcon from '~/components/icons/FolderIcon';
import * as RNFS from 'react-native-fs';
import { Location, useLibraryMutation, useLibraryQuery } from '@sd/client';
import DocumentPicker from 'react-native-document-picker';
import { SharedScreenProps } from '~/navigation/SharedScreens';

// Add more default locations here?
const defaultLocationsList: { name: string, absPath: string }[] = [{ name: 'Downloads', absPath: RNFS.DownloadDirectoryPath }, { name: 'Placeholder', absPath: 'placeholder' }]
const LocationOnboarding = ({ navigation }: SharedScreenProps<'LocationOnboarding'>) => {
	const addLocationToLibrary = useLibraryMutation('locations.addLibrary');
	const relinkLocation = useLibraryMutation('locations.relink');
	const { data: _locations, refetch: refetchLocations } = useLibraryQuery(['locations.list']);
	const [locations, setLocations] = React.useState<Location[] | undefined>(_locations);

	const createLocation = useLibraryMutation('locations.create', {
		onError: (error, variables) => {
			switch (error.message) {
				case 'NEED_RELINK':
					if (!variables.dry_run) relinkLocation.mutate(variables.path);
					break;
				case 'ADD_LIBRARY':
					addLocationToLibrary.mutate(variables);
					break;
				default:
					throw new Error('Unimplemented custom remote error handling');
			}
		},
		onSuccess: async () => {
			const refreshedLocations = await refetchLocations();
			setLocations(refreshedLocations.data);
		}
	});


	const handleFilesButton = useCallback(async () => {
		try {
			const response = await DocumentPicker.pickDirectory({
				presentationStyle: 'pageSheet'
			});

			if (!response) return;

			const uri = response.uri;

			if (Platform.OS === 'android') {
				// The following code turns this: content://com.android.externalstorage.documents/tree/[filePath] into this: /storage/emulated/0/[directoryName]
				// Example: content://com.android.externalstorage.documents/tree/primary%3ADownload%2Ftest into /storage/emulated/0/Download/test
				const dirName = decodeURIComponent(uri).split('/');
				// Remove all elements before 'tree'
				dirName.splice(0, dirName.indexOf('tree') + 1);
				const parsedDirName = dirName.join('/').split(':')[1];
				const dirPath = RNFS.ExternalStorageDirectoryPath + '/' + parsedDirName;
				//Verify that the directory exists
				const dirExists = await RNFS.exists(dirPath);
				if (!dirExists) {
					console.error('Directory does not exist'); //TODO: Make this a UI error
					return;
				}

				createLocation.mutate({
					path: dirPath,
					dry_run: false,
					indexer_rules_ids: []
				});
				// Get the id of the location we just added
				const newLocation = locations?.filter((location: Location) => location.path === dirPath)[0];
				if (!newLocation) return;
				// Navigate to the location
				navigation.navigate('Location', {
					id: newLocation?.id
				});
			} else {
				// iOS
				createLocation.mutate({
					path: decodeURIComponent(uri.replace('file://', '')),
					dry_run: false,
					indexer_rules_ids: []
				});
			}
		} catch (err) {
			console.error(err);
		}
	}, [createLocation]);

	return (
		<View style={tw`flex-1 items-start justify-start p-5`}>
			<View style={tw`mt-2`}>
				{defaultLocationsList?.map(({ name, absPath }) => (
					<Pressable onPress={async () => {
						if (absPath === 'placeholder') {
							ToastAndroid.showWithGravity(
								`This location is a placeholder`,
								ToastAndroid.SHORT,
								ToastAndroid.CENTER
							);
							return;
						}
						createLocation.mutate({
							path: absPath,
							dry_run: false,
							indexer_rules_ids: []
						});
						ToastAndroid.showWithGravity(
							`Added ${name} to Library`,
							ToastAndroid.SHORT,
							ToastAndroid.CENTER
						);

						// Get the id of the location we just added
						const newLocation = locations?.filter((location: Location) => location.path === absPath)[0];
						if (!newLocation) return;
						// Navigate to the location
						navigation.navigate('Location', {
							id: newLocation?.id
						});
					}} key={name}>
						<View style={twStyle('mb-[4px] flex flex-row items-center rounded px-1 py-2')}>
							<FolderIcon size={50} />
							<Text style={twStyle('ml-1.5 text-xl text-gray-300')} numberOfLines={1}>
								{name}
							</Text>
						</View>
					</Pressable>
				))}
				<Pressable onPress={handleFilesButton}>
					<View style={twStyle('mb-[4px] flex flex-row items-center rounded px-1 py-2')}>
						<FolderIcon size={50} />
						<Text style={twStyle('ml-1.5 text-xl text-gray-300')} numberOfLines={1}>
							Add other location
						</Text>
					</View>
				</Pressable>
			</View>
		</View>
	);
};
export default LocationOnboarding;
