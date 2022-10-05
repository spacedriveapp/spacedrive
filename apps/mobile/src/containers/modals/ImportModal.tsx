import { BottomSheetModal } from '@gorhom/bottom-sheet';
import * as MediaLibrary from 'expo-media-library';
import React, { forwardRef, useCallback } from 'react';
import { Text, View } from 'react-native';
import DocumentPicker from 'react-native-document-picker';
import { Modal } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import { useLibraryMutation } from '~/hooks/rspc';
import tw from '~/lib/tailwind';

const ImportModal = forwardRef<BottomSheetModal, unknown>((_, ref) => {
	const { mutate: createLocation } = useLibraryMutation('locations.create', {
		onError(error, variables, context) {
			// TODO: Toast message
			console.log(error);
		}
	});

	const handleFilesButton = useCallback(async () => {
		try {
			const response = await DocumentPicker.pickDirectory({
				presentationStyle: 'pageSheet'
			});
			createLocation({
				path: response.uri.replace('file://', '').replaceAll('%20', ' '), //TODO: Parse path better...
				indexer_rules_ids: []
			});
		} catch (err) {
			// console.warn(err);
		}
	}, [createLocation]);

	// const [status, requestPermission] = MediaLibrary.usePermissions();
	// console.log(status);

	const handlePhotosButton = useCallback(async () => {
		const permission = await MediaLibrary.getPermissionsAsync();
		console.log(permission);

		const assets = await MediaLibrary.getAssetsAsync({ mediaType: MediaLibrary.MediaType.photo });
		assets.assets.map(async (i) => {
			console.log(await MediaLibrary.getAssetInfoAsync(i));
		});
		// console.log(await MediaLibrary.getAssetInfoAsync({id: }))
	}, []);

	return (
		<Modal ref={ref} snapPoints={['20%']}>
			<View style={tw`flex-1 px-6 pt-1 pb-2 bg-gray-600`}>
				<Button size="md" variant="primary" style={tw`my-2`} onPress={handleFilesButton}>
					<Text>Import from Files</Text>
				</Button>
				<Button size="md" variant="primary" onPress={handlePhotosButton}>
					<Text>Import from Photos</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default ImportModal;
