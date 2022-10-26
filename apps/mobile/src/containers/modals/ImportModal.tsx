import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { useLibraryMutation } from '@sd/client';
import * as ML from 'expo-media-library';
import { forwardRef, useCallback } from 'react';
import { Alert, Platform, Text, View } from 'react-native';
import DocumentPicker from 'react-native-document-picker';
import RFS from 'react-native-fs';
import { Modal } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

const ImportModal = forwardRef<BottomSheetModal, unknown>((_, ref) => {
	const modalRef = useForwardedRef(ref);

	const { mutate: createLocation } = useLibraryMutation('locations.create', {
		onError: (error, variables, context) => {
			console.error(error);
		},
		onSettled: (data, error, variables, context) => {
			// Close the modal
			modalRef.current?.close();
		}
	});

	const handleFilesButton = useCallback(async () => {
		try {
			const response = await DocumentPicker.pickDirectory({
				presentationStyle: 'pageSheet'
			});

			createLocation({
				path: decodeURIComponent(response.uri.replace('file://', '')),
				indexer_rules_ids: []
			});
		} catch (err) {
			console.error(err);
		}
	}, [createLocation]);

	// Temporary until we decide on the user flow
	const handlePhotosButton = useCallback(async () => {
		// Check if we have full access to the photos library
		let permission = await ML.getPermissionsAsync();
		// {"accessPrivileges": "none", "canAskAgain": true, "expires": "never", "granted": false, "status": "undetermined"}

		if (
			permission.status === ML.PermissionStatus.UNDETERMINED ||
			(permission.status === ML.PermissionStatus.DENIED && permission.canAskAgain)
		) {
			permission = await ML.requestPermissionsAsync();
		}

		// Permission Denied
		if (permission.status === ML.PermissionStatus.DENIED) {
			Alert.alert(
				'Permission required',
				'You need to grant access to your photos library to import your photos/videos.'
			);
			return;
		}

		// Limited Permission (Can't access path)
		if (permission.accessPrivileges === 'limited') {
			Alert.alert(
				'Limited access',
				'You need to grant full access to your photos library to import your photos/videos.'
			);
			return;
		}

		// Permission Granted
		// TODO: Find the paths??
		const libraryPath = Platform.select({
			android: '',
			ios: RFS.MainBundlePath + '/Media/DCIM'
		});

		createLocation({
			path: libraryPath,
			indexer_rules_ids: []
		});

		// const assets = await ML.getAssetsAsync({ mediaType: ML.MediaType.photo });
		// assets.assets.map(async (i) => {
		// 	console.log((await ML.getAssetInfoAsync(i)).localUri);
		// });
	}, [createLocation]);

	// const testFN = useCallback(async () => {
	// 	const URL = decodeURIComponent(RFS.DocumentDirectoryPath + '/libraries');
	// 	RFS.readdir(URL).then((files) => {
	// 		files.forEach((file) => {
	// 			console.log(file);
	// 		});
	// 	});
	// }, []);

	return (
		<Modal ref={ref} snapPoints={['20%']}>
			<View style={tw`flex-1 px-6 pt-1 pb-2 bg-gray-600`}>
				{/* <Button size="md" variant="primary" style={tw`my-2`} onPress={testFN}>
					<Text>TEST</Text>
				</Button> */}
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
